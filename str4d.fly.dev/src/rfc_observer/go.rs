use askama::Template;
use askama_web::WebTemplate;
use axum::{routing::get, Json, Router};
use cached::proc_macro::cached;

mod data;
mod github;

pub(crate) fn build() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/data", get(data))
}

#[derive(Clone, Template, WebTemplate)]
#[template(path = "rfc.observer/go.html")]
struct Index {}

async fn index() -> Index {
    Index {}
}

#[cached(time = 600)]
async fn data() -> Json<Option<data::Data>> {
    let data = match self::github::get_proposals().await {
        Ok(proposals) => Some(data::Data::new(proposals)),
        Err(e) => {
            tracing::error!("Failed to get proposals: {}", e);
            None
        }
    };

    Json(data)
}
