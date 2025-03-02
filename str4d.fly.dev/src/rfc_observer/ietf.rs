use std::sync::Arc;

use askama::Template;
use axum::{
    extract::{Path, State},
    routing::get,
    Json, Router,
};
use hyper::StatusCode;

mod data;
mod datatracker;

pub(crate) fn build() -> Router {
    let state = Arc::new(self::datatracker::build_client().expect("should succeed"));

    Router::new()
        .route("/", get(index))
        .route("/:acronym", get(group))
        .route("/api/data/:acronym", get(data))
        .with_state(state)
}

#[derive(Clone, Template)]
#[template(path = "rfc.observer/ietf.html")]
struct Index {
    active_groups: Vec<self::datatracker::Group>,
    inactive_groups: Vec<self::datatracker::Group>,
}

async fn index(State(client): State<Arc<reqwest::Client>>) -> Result<Index, StatusCode> {
    self::datatracker::get_groups(&client)
        .await
        .map(|(active_groups, inactive_groups)| Index {
            active_groups,
            inactive_groups,
        })
        .map_err(|e| {
            tracing::error!("Failed to get groups: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

#[derive(Clone, Template)]
#[template(path = "rfc.observer/ietf-group.html")]
struct Group {
    group: self::datatracker::Group,
}

async fn group(
    State(client): State<Arc<reqwest::Client>>,
    Path(acronym): Path<String>,
) -> Result<Group, StatusCode> {
    self::datatracker::get_group(&client, &acronym)
        .await
        .map(|group| Group { group })
        .map_err(|e| {
            tracing::error!("Failed to get group: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

async fn data(
    State(client): State<Arc<reqwest::Client>>,
    Path(acronym): Path<String>,
) -> Json<Option<data::Data>> {
    let data = match self::datatracker::get_documents(&client, &acronym).await {
        Ok(documents) => Some(data::Data::new(documents)),
        Err(e) => {
            tracing::error!("Failed to get documents: {:?}", e);
            None
        }
    };

    Json(data)
}
