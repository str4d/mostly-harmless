use atrium_api::{
    agent::atp_agent::{AtpAgent, store::MemorySessionStore},
    app::bsky::feed::get_suggested_feeds,
};
use atrium_xrpc_client::reqwest::ReqwestClient;

use super::Error;

pub(super) async fn enumerate(
    bsky: &AtpAgent<MemorySessionStore, ReqwestClient>,
) -> Result<Vec<super::Feed>, Error> {
    let mut feeds = vec![];
    let mut cursor = None;

    loop {
        let response = bsky
            .api
            .app
            .bsky
            .feed
            .get_suggested_feeds(
                get_suggested_feeds::ParametersData {
                    limit: Some(100.try_into().expect("valid")),
                    cursor,
                }
                .into(),
            )
            .await?;
        tracing::debug!("Loaded {} feeds", response.feeds.len());

        feeds.extend(response.data.feeds.into_iter().map(|feed| super::Feed {
            name: feed.data.display_name,
            likes: feed.data.like_count.unwrap_or(0),
            bsky_operated: feed.data.creator.handle.ends_with("bsky.app"),
        }));

        cursor = response.data.cursor;

        if cursor.is_none() {
            break;
        }
    }

    Ok(feeds)
}
