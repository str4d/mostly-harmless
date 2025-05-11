use std::collections::{HashMap, HashSet};
use std::env;

use atrium_api::agent::atp_agent::{AtpAgent, store::MemorySessionStore};
use atrium_xrpc_client::reqwest::{ReqwestClient, ReqwestClientBuilder};
use tracing::warn;

use super::Error;

mod feed;
mod labeler;
mod pds;

pub(super) async fn enumerate(client: &reqwest::Client) -> Result<Network, Error> {
    // Hard-coded list of known relays (they aren't discoverable).
    let relays = vec![
        Relay::new("Bluesky Relay US East", "US", "relay1.us-east.bsky.network"),
        Relay::new("Bluesky Relay US West", "US", "relay1.us-west.bsky.network"),
        Relay::new("Blacksky Relay US", "US", "atproto.africa"),
        Relay::new("Cerulea Relay EU", "EU", "relay.cerulea.blue"),
        Relay::new("Firehose Cam CA", "CA", "relay.fire.hose.cam"),
        Relay::new("Firehose Cam EU", "EU", "relay3.fr.hose.cam"),
    ];

    let bsky = sign_in(client).await?;

    let mut pdss = HashMap::new();
    for (relay_index, relay) in relays.iter().enumerate() {
        let relay_pdss = match pds::enumerate(client, relay).await {
            Ok(pdss) => pdss,
            Err(e) => {
                warn!("Failed to enumerate PDSs on {}: {:?}", relay.name, e);
                continue;
            }
        };
        for (hostname, pds) in relay_pdss {
            pdss.entry(hostname)
                .and_modify(|cur: &mut Pds| {
                    cur.account_count = cur.account_count.max(pds.account_count);
                })
                .or_insert(pds)
                .relays
                .insert(relay_index);
        }
    }

    let labelers = labeler::enumerate(client, &bsky).await?;
    let feeds = feed::enumerate(&bsky).await?;

    Ok(Network {
        relays,
        pdss,
        labelers,
        feeds,
    })
}

async fn sign_in(
    client: &reqwest::Client,
) -> Result<AtpAgent<MemorySessionStore, ReqwestClient>, Error> {
    let password = env::var("BLUESKY_APP_PASSWORD").map_err(|_| Error::BlueskyAuthRequired)?;

    // Sign in to Bluesky
    let client = AtpAgent::new(
        ReqwestClientBuilder::new("https://bsky.social")
            .client(client.clone())
            .build(),
        MemorySessionStore::default(),
    );
    client
        .login("str4d.bsky.social", &password)
        .await
        .map_err(|e| {
            tracing::error!("Failed to log in: {}", e);
            Error::BlueskyAuthRequired
        })?;

    Ok(client)
}

#[derive(Debug)]
pub(super) struct Network {
    pub(super) relays: Vec<Relay>,
    pub(super) pdss: HashMap<String, Pds>,
    pub(super) labelers: Vec<Labeler>,
    pub(super) feeds: Vec<Feed>,
}

#[derive(Debug)]
pub(super) struct Relay {
    pub(super) name: &'static str,
    pub(super) region: &'static str,
    pub(super) host: &'static str,
    pub(super) bsky_operated: bool,
}

impl Relay {
    fn new(name: &'static str, region: &'static str, host: &'static str) -> Self {
        Self {
            name,
            region,
            host,
            bsky_operated: host.ends_with(".bsky.network"),
        }
    }
}

#[derive(Debug)]
pub(super) struct Pds {
    pub(super) relays: HashSet<usize>,
    pub(super) account_count: usize,
    pub(super) status: String,
}

#[derive(Debug)]
pub(super) struct Labeler {
    pub(super) name: String,
    pub(super) likes: usize,
    pub(super) bsky_operated: bool,
}

#[derive(Debug)]
pub(super) struct Feed {
    pub(super) name: String,
    pub(super) likes: usize,
    pub(super) bsky_operated: bool,
}
