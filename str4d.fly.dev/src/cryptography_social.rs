use std::collections::HashMap;

use anyhow::Context;
use askama::Template;
use askama_web::WebTemplate;
use axum::{Router, routing::get};
use cached::proc_macro::cached;
use serde::Deserialize;

pub(crate) fn build() -> Router {
    Router::new().route("/", get(index))
}

#[derive(Template, WebTemplate)]
#[template(path = "cryptography.social/index.html")]
struct Index {
    users: Vec<User>,
}

async fn index() -> Index {
    let users = match fetch_eprint_authors().await {
        Ok(users) => users,
        Err(e) => {
            tracing::error!("Failed to fetch ePrint authors: {e}");
            vec![]
        }
    };

    Index { users }
}

#[derive(Clone)]
struct User {
    name: String,
    profile: String,
    avatar: Option<String>,
}

impl User {
    fn new(name: &str, did: &str, avatar: Option<String>) -> Self {
        Self {
            name: name.into(),
            profile: format!("https://bsky.app/profile/{did}"),
            avatar: avatar.map(|avatar| avatar.replace("img/avatar", "img/avatar_thumbnail")),
        }
    }
}

#[cached(result = true, time = 60)]
async fn fetch_eprint_authors() -> Result<Vec<User>, anyhow::Error> {
    let client = reqwest::ClientBuilder::new().use_rustls_tls().build()?;

    let list = client
        .get("http://app.process.str4d-bots.internal:9001")
        .send()
        .await
        .context("Failed to fetch authors list")?
        .error_for_status()?
        .json::<Authors>()
        .await
        .context("Failed to decode authors list")?;

    let mut authors = vec![];
    for chunk in list.authors.chunks(25) {
        let mut request_url =
            String::from("https://public.api.bsky.app/xrpc/app.bsky.actor.getProfiles?actors=");
        for (i, author) in chunk.iter().enumerate() {
            if i > 0 {
                request_url += "&actors=";
            }
            request_url += &author.did;
        }

        let response = client
            .get(request_url)
            .send()
            .await
            .context("Failed to fetch authors profiles")?
            .error_for_status()?
            .json::<ProfilesResponse>()
            .await
            .context("Failed to decode authors profiles")?;

        match response {
            ProfilesResponse::Success(obj) => {
                let mut profiles = obj
                    .profiles
                    .into_iter()
                    .map(|p| (p.did, p.avatar))
                    .collect::<HashMap<_, _>>();

                for author in chunk {
                    let avatar = profiles.remove(&author.did).flatten();
                    authors.push(User::new(&author.name, &author.did, avatar));
                }
            }
            ProfilesResponse::Failure(err) => {
                tracing::error!("Profile request failed ({}): {}", err.error, err.message);
            }
        }
    }

    // Make the list easier to read.
    authors.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(authors)
}

#[derive(Deserialize)]
struct Authors {
    authors: Vec<Author>,
}

#[derive(Deserialize)]
struct Author {
    name: String,
    did: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ProfilesResponse {
    Success(Profiles),
    Failure(XrpcError),
}

#[derive(Deserialize)]
struct Profiles {
    profiles: Vec<Profile>,
}

#[derive(Deserialize)]
struct Profile {
    did: String,
    avatar: Option<String>,
}

#[derive(Deserialize)]
struct XrpcError {
    error: String,
    message: String,
}
