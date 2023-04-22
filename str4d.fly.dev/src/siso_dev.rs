use askama::Template;
use axum::{routing::get, Router};
use cached::proc_macro::cached;

mod atproto;

pub(crate) fn build() -> Router {
    Router::new().route("/", get(index))
}

#[derive(Clone, Template)]
#[template(path = "siso.dev/index.html")]
struct Index {
    feed: Vec<atproto::Post>,
}

#[cached(time = 60)]
async fn index() -> Index {
    let feed = match atproto::Client::new().get_feed().await {
        Ok(feed) => feed,
        Err(e) => {
            tracing::error!("Failed to get feed: {:?}", e);
            vec![]
        }
    };
    Index { feed }
}
