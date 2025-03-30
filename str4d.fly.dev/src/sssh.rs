use askama::Template;
use askama_web::WebTemplate;
use axum::{Router, routing::get};

pub(crate) fn build() -> Router {
    Router::new().route("/", get(index))
}

#[derive(Clone, Template, WebTemplate)]
#[template(path = "s-s.sh/index.html")]
struct Index {}

async fn index() -> Index {
    Index {}
}
