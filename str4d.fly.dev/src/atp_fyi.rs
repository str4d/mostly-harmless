use askama::Template;
use axum::{routing::get, Router};
use cached::proc_macro::cached;

mod github;
pub(crate) mod network;

pub(crate) fn build() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/roadmap", get(roadmap))
}

#[derive(Clone, Template)]
#[template(path = "atp.fyi/index.html")]
struct Index {}

async fn index() -> Index {
    Index {}
}

#[derive(Clone, Template)]
#[template(path = "atp.fyi/roadmap.html")]
struct Roadmap {
    roadmap: Option<github::Roadmap>,
}

#[cached(time = 60)]
async fn roadmap() -> Roadmap {
    let roadmap = match self::github::get_roadmap().await {
        Ok(roadmap) => Some(roadmap),
        Err(e) => {
            tracing::error!("Failed to get roadmap: {:?}", e);
            None
        }
    };
    Roadmap { roadmap }
}
