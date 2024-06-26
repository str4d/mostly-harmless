use askama::Template;
use axum::{routing::get, Router};

mod data;
mod github;

pub(crate) fn build() -> Router {
    Router::new().route("/", get(index))
}

#[derive(Clone, Template)]
#[template(path = "rfc.observer/rust/index.html")]
struct Index {
    data: Option<data::Data>,
}

async fn index() -> Index {
    let data = match self::github::get_tracking_issues().await {
        Ok(tracking_issues) => Some(data::Data::new(tracking_issues)),
        Err(e) => {
            tracing::error!("Failed to get tracking issues: {:?}", e);
            None
        }
    };

    Index { data }
}
