use askama::Template;
use askama_web::WebTemplate;
use axum::{routing::get, Router};

pub(crate) mod common;

pub(crate) mod go;
pub(crate) mod ietf;
pub(crate) mod rust;

pub(crate) fn build() -> Router {
    Router::new().route("/", get(index))
}

#[derive(Clone, Template, WebTemplate)]
#[template(path = "rfc.observer/index.html")]
struct Index {}

async fn index() -> Index {
    Index {}
}
