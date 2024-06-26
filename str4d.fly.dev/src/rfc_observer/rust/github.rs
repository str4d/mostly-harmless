use graphql_client::GraphQLQuery;

use crate::util::github;

use super::data::TrackingIssue;

type DateTime = chrono::DateTime<chrono::Utc>;

pub(super) async fn get_tracking_issues() -> Result<Vec<TrackingIssue>, Error> {
    let client = github::Client::new("rust.rfc.observer")?;

    let data = client
        .post_paginated_graphql::<RustRfcQuery>(rust_rfc_query::Variables { after: None })
        .await?
        .into_data()
        .map_err(Error::GraphQl)?;

    let repo = data.repository.expect("repo exists");

    let mut tracking_issues = repo
        .issues
        .edges
        .into_iter()
        .flat_map(|issues| issues.into_iter())
        .map(|e| e.and_then(|edge| edge.node))
        .flatten()
        .flat_map(|issue| TrackingIssue::new(issue))
        .collect::<Vec<_>>();

    tracking_issues.sort_by_key(|issue| (issue.rfc, issue.created_at, issue.closed_at));

    Ok(tracking_issues)
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "res/graphql/github-schema.graphql",
    query_path = "res/graphql/rust-rfc-query.graphql"
)]
pub struct RustRfcQuery;

impl github::PaginatedQuery for RustRfcQuery {
    fn page_info(data: &Self::ResponseData) -> github::PageInfo {
        let page_info = &data
            .repository
            .as_ref()
            .expect("repo exists")
            .issues
            .page_info;

        github::PageInfo {
            end_cursor: page_info.end_cursor.clone(),
            has_next_page: page_info.has_next_page,
        }
    }

    fn with_after(_: &Self::Variables, after: Option<String>) -> Self::Variables {
        rust_rfc_query::Variables { after }
    }

    fn merge_page(acc: &mut Self::ResponseData, page: Self::ResponseData) {
        let issues = &mut acc.repository.as_mut().expect("repo exists").issues;

        match (
            issues.edges.as_mut(),
            page.repository.expect("repo exists").issues.edges,
        ) {
            (_, None) => (),
            (None, Some(edges)) => issues.edges = Some(edges),
            (Some(acc), Some(mut page)) => acc.append(&mut page),
        }
    }
}

#[derive(Debug)]
pub(super) enum Error {
    GitHub(github::Error),
    GraphQl(Vec<graphql_client::Error>),
}

impl From<github::Error> for Error {
    fn from(err: github::Error) -> Self {
        Error::GitHub(err)
    }
}
