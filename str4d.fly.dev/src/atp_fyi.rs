use askama::Template;
use axum::{routing::get, Router};

pub(crate) fn build() -> Router {
    Router::new().route("/", get(index))
}

#[derive(Clone, Template)]
#[template(path = "atp.fyi/index.html")]
struct Index {}

async fn index() -> Index {
    Index {}
}
