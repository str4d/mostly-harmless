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
) -> Result<Vec<(String, usize)>, Error> {
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
                // Ignore labelers that have no labels (and thus aren't affecting AppViews).
                if labeler.policies.label_values.is_empty() {
                    None
                } else {
                    let likes = labeler.data.like_count.unwrap_or(0);

                    // Ignore labelers that have no name or valid handle.
                    match labeler.data.creator.data.display_name {
                        Some(name) if !name.trim().is_empty() => Some((name, likes)),
                        _ => match labeler.creator.handle.as_str() {
                            "handle.invalid" => None,
                            _ => Some((labeler.creator.handle.to_string(), likes)),
                        },
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
