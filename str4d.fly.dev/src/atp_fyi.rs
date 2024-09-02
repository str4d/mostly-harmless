use std::time::Duration;

use askama::Template;
use askama_web::WebTemplate;
use axum::{Extension, Json, Router, routing::get};
use cached::proc_macro::cached;

mod github;
pub(crate) mod network;

pub(crate) fn build() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/network", get(network))
        .route("/roadmap", get(roadmap))
        .route("/api/network-map", get(network_map))
}

#[derive(Clone, Template, WebTemplate)]
#[template(path = "atp.fyi/index.html")]
struct Index {
    rates: Option<(network::firehose::FirehoseRate, Duration)>,
}

async fn index() -> Index {
    Index {
        rates: network::firehose::average_rates_per_min().await,
    }
}

#[derive(Clone, Template, WebTemplate)]
#[template(path = "atp.fyi/network.html")]
struct Network {}

async fn network() -> Network {
    Network {}
}

#[cached(time = 600, key = "()", convert = r##"{}"##)]
async fn network_map(Extension(client): Extension<reqwest::Client>) -> Json<Option<network::Map>> {
    let map = match self::network::render_map(&client).await {
        Ok(map) => Some(map),
        Err(e) => {
            tracing::error!("Failed to render network map: {:?}", e);
            None
        }
    };

    Json(map)
}

#[derive(Clone, Template, WebTemplate)]
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

mod filters {
    use std::time::Duration;

    pub fn fmtduration(d: &Duration, _: &dyn askama::Values) -> ::askama::Result<String> {
        let d = chrono::TimeDelta::from_std(*d).map_err(|e| askama::Error::Custom(Box::new(e)))?;
        let half_past = d.num_minutes() >= 30;

        match d.num_hours() {
            0 => match d.num_minutes() {
                1 => Ok("minute".into()),
                n => Ok(format!("{} minutes", n)),
            },
            1 if !half_past => Ok("hour".into()),
            n => Ok(format!("{} hours", n + if half_past { 1 } else { 0 })),
        }
    }

    pub fn fmtf64(value: &f64, _: &dyn askama::Values) -> ::askama::Result<String> {
        if *value >= 100.0 {
            Ok(format!("{value:.0}"))
        } else {
            Ok(format!("{value:.3}"))
        }
    }
}
