use std::collections::HashSet;
use std::fmt;
use std::iter;

use axum::http::HeaderValue;
use cached::proc_macro::cached;
use chrono::{DateTime, Utc};
use hyper::header::ACCEPT;
use serde::{de::DeserializeOwned, Deserialize, Deserializer, Serialize};

pub(super) fn build_client() -> Result<reqwest::Client, Error> {
    Ok(reqwest::Client::builder()
        .user_agent("ietf.rfc.observer")
        .default_headers(
            iter::once((ACCEPT, HeaderValue::from_static("application/json"))).collect(),
        )
        .build()?)
}

async fn get<T: DeserializeOwned>(client: &reqwest::Client, path: &str) -> Result<T, Error> {
    Ok(client
        .get(format!("https://datatracker.ietf.org{path}"))
        .send()
        .await?
        .error_for_status()?
        .json::<T>()
        .await?)
}

pub(super) async fn get_paginated<T: DeserializeOwned>(
    client: &reqwest::Client,
    path: &str,
) -> Result<Vec<T>, Error> {
    let res = get::<ListResult<T>>(client, &format!("{path}&limit=0")).await?;

    let mut objects = Vec::with_capacity(res.meta.total_count as usize);
    objects.extend(res.objects);

    let mut next = res.meta.next;
    while let Some(path) = next {
        let res = get::<ListResult<T>>(client, &path).await?;
        objects.extend(res.objects);
        next = res.meta.next;
    }

    Ok(objects)
}

#[cached(
    time = 600,
    result = true,
    key = "String",
    convert = r#"{ String::from("IETF-Groups") }"#
)]
pub(super) async fn get_groups(
    client: &reqwest::Client,
) -> Result<(Vec<Group>, Vec<Group>), Error> {
    // From a previous scan, the following group types have I-Ds or RFCs:
    // - ag
    // - area
    // - edwg
    // - iana
    // - ietf
    // - individ
    // - rag
    // - rg
    // - wg
    let mut groups = get_paginated::<Group>(client, "/api/v1/group/group/?type__in=ag&type__in=area&type__in=edwg&type__in=iana&type__in=ietf&type__in=individ&type__in=rag&type__in=rg&type__in=wg").await?;

    groups.sort_by(|a, b| a.acronym.cmp(&b.acronym));

    // Split into active and inactive.
    let mut active_groups = vec![];
    let mut inactive_groups = vec![];
    for group in groups {
        if group.state == "/api/v1/name/groupstatename/active/" {
            active_groups.push(group);
        } else {
            inactive_groups.push(group);
        }
    }

    Ok((active_groups, inactive_groups))
}

#[cached(
    time = 600,
    result = true,
    key = "String",
    convert = r#"{ String::from(acronym) }"#
)]
pub(super) async fn get_group(client: &reqwest::Client, acronym: &str) -> Result<Group, Error> {
    let group_res =
        get::<ListResult<Group>>(client, &format!("/api/v1/group/group/?acronym={acronym}"))
            .await?;
    assert_eq!(group_res.objects.len(), 1);
    Ok(group_res.objects.into_iter().next().expect("present"))
}

#[cached(
    time = 600,
    result = true,
    key = "String",
    convert = r#"{ String::from(acronym) }"#
)]
pub(super) async fn get_documents(
    client: &reqwest::Client,
    acronym: &str,
) -> Result<Vec<super::data::Document>, Error> {
    let group = get_group(&client, acronym).await?;

    // Fetch the documents belonging to this group.
    let rfcs = get_paginated::<Document>(
        client,
        &format!("/api/v1/doc/document/?group={}&type=rfc", group.id),
    )
    .await?;
    let mut drafts = get_paginated::<Document>(
        client,
        &format!("/api/v1/doc/document/?group={}&type=draft", group.id),
    )
    .await?;

    // Ignore drafts that have been replaced by subsequent documents.
    let states_to_ignore = [
        "/api/v1/doc/state/3/".into(),   // draft - RFC
        "/api/v1/doc/state/4/".into(),   // draft - Replaced
        "/api/v1/doc/state/7/".into(),   // draft-iesg - RFC Published
        "/api/v1/doc/state/53/".into(),  // draft-stream-iab - Published RFC
        "/api/v1/doc/state/65/".into(),  // draft-stream-irtf - Published RFC
        "/api/v1/doc/state/74/".into(),  // draft-stream-ise - Published RFC
        "/api/v1/doc/state/147/".into(), // draft-stream-iab - Replaced
        "/api/v1/doc/state/148/".into(), // draft-stream-ise - Replaced
        "/api/v1/doc/state/149/".into(), // draft-stream-irtf - Replaced
        "/api/v1/doc/state/170/".into(), // draft-stream-editorial - Replaced editorial stream document
        "/api/v1/doc/state/173/".into(), // draft-stream-editorial - Published RFC
        "/api/v1/doc/state/176/".into(), // statement - Replaced
        "/api/v1/doc/state/177/".into(), // rfc - Published
    ]
    .into_iter()
    .collect::<HashSet<_>>();
    drafts.retain(|doc| doc.states.is_disjoint(&states_to_ignore));

    // Hydrate the documents.
    let mut docs = vec![];

    for rfc in rfcs {
        let rfc_number = rfc.rfc_number.ok_or(Error::MalformedResponse)?;
        let info = get::<DocInfo>(client, &format!("/doc/{}/doc.json", rfc.name)).await?;
        if let Some(doc) = super::data::Document::new(Some(rfc_number), info, None) {
            docs.push(doc);
        }
    }

    for draft in drafts {
        let info = get::<DocInfo>(client, &format!("/doc/{}/doc.json", draft.name)).await?;
        if let Some(doc) = super::data::Document::new(None, info, draft.expires) {
            docs.push(doc);
        }
    }

    Ok(docs)
}

#[derive(Debug, Deserialize)]
struct ListResult<T> {
    meta: ListMeta,
    objects: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct ListMeta {
    next: Option<String>,
    total_count: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Group {
    id: u64,
    pub(super) acronym: String,
    pub(super) name: String,
    description: String,
    #[serde(rename = "type")]
    pub(super) kind: String,
    pub(super) state: String,
}

#[derive(Debug, Deserialize)]
struct Document {
    name: String,
    rfc_number: Option<u32>,
    #[serde(deserialize_with = "deserialize_optional_time")]
    expires: Option<DateTime<Utc>>,
    states: HashSet<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct DocInfo {
    pub(super) name: String,
    pub(super) title: String,
    pub(super) rev_history: Vec<Revision>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Revision {
    #[serde(deserialize_with = "deserialize_time")]
    pub(super) published: DateTime<Utc>,
}

fn deserialize_time<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(DateTime::parse_from_str(&s, "%+")
        .map_err(serde::de::Error::custom)?
        .to_utc())
}

fn deserialize_optional_time<'de, D>(deserializer: D) -> Result<Option<DateTime<Utc>>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = Option::<String>::deserialize(deserializer)?;
    Ok(s.map(|s| {
        DateTime::parse_from_str(&s, "%+")
            .map(|t| t.to_utc())
            .map_err(serde::de::Error::custom)
    })
    .transpose()?)
}

#[derive(Debug)]
pub(super) enum Error {
    MalformedResponse,
    Request(reqwest::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::MalformedResponse => write!(f, "Response from datatracker was malformed"),
            Error::Request(e) => write!(f, "Error while processing request: {}", e),
        }
    }
}

impl std::error::Error for Error {}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::Request(err)
    }
}
