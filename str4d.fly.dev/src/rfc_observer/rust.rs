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
#[template(path = "rfc.observer/rust.html")]
struct Index {}

async fn index() -> Index {
    Index {}
}

#[cached(time = 600)]
async fn data() -> Json<Option<data::Data>> {
    let data = match self::github::get_tracking_issues().await {
        Ok(tracking_issues) => Some(data::Data::new(tracking_issues)),
        Err(e) => {
            tracing::error!("Failed to get tracking issues: {}", e);
            None
        }
    };

    Json(data)
}
