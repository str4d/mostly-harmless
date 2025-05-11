use atrium_api::{
    agent::atp_agent::{AtpAgent, store::MemorySessionStore},
    app::bsky::labeler::get_services,
    types::{Union, string::Did},
};
use atrium_xrpc_client::reqwest::ReqwestClient;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::Error;

pub(super) async fn enumerate(
    client: &reqwest::Client,
    bsky: &AtpAgent<MemorySessionStore, ReqwestClient>,
) -> Result<Vec<super::Labeler>, Error> {
    let response = client
        .get("https://blue.mackuba.eu/xrpc/blue.feeds.mod.getLabellers")
        .send()
        .await?
        .error_for_status()?
        .json::<LabelersResponse>()
        .await?;

    let labeler_dids = response
        .labellers
        .into_iter()
        .map(|labeler| labeler.did)
        .collect::<Vec<_>>();

    let mut labelers = Vec::with_capacity(labeler_dids.len());

    for chunk in labeler_dids.chunks(200) {
        let labelers_info = bsky
            .api
            .app
            .bsky
            .labeler
            .get_services(
                get_services::ParametersData {
                    dids: chunk.to_vec(),
                    detailed: Some(true),
                }
                .into(),
            )
            .await?
            .data
            .views
            .into_iter()
            .map(|view| match view {
                Union::Refs(
                    get_services::OutputViewsItem::AppBskyLabelerDefsLabelerViewDetailed(labeler),
                ) => labeler,
                _ => unreachable!(),
            })
            .filter_map(|labeler| {
                let handle = labeler.creator.handle.to_string();
                let bsky_operated = handle.ends_with(".bsky.app");

                // Ignore labelers that have no labels (and thus aren't affecting AppViews)
                // except for Bluesky-operated labelers (some of which only serve regional
                // labels).
                if labeler.policies.label_values.is_empty() && !bsky_operated {
                    None
                } else {
                    let name = labeler
                        .data
                        .creator
                        .data
                        .display_name
                        .filter(|name| !name.trim().is_empty())
                        .unwrap_or(handle.clone());
                    let likes = labeler.data.like_count.unwrap_or(0);

                    // Ignore labelers that have no valid handle.
                    match handle.as_str() {
                        "handle.invalid" => None,
                        _ => Some(super::Labeler {
                            name,
                            likes,
                            bsky_operated,
                        }),
                    }
                }
            });

        labelers.extend(labelers_info);
    }

    Ok(labelers)
}

#[derive(Debug, Deserialize)]
struct LabelersResponse {
    labellers: Vec<Labeler>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Labeler {
    id: u32,
    did: Did,
    name: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    handle: String,
    endpoint: Option<String>,
}
