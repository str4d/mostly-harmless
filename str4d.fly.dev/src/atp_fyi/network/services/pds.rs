use serde::Deserialize;

use super::{Error, Pds, Relay};

pub(super) async fn enumerate(
    client: &reqwest::Client,
    relay: &Relay,
) -> Result<Vec<(String, Pds)>, Error> {
    let response = list_hosts(client, relay, None).await?;
    let mut pdss = response
        .hosts
        .into_iter()
        .map(|host| host.into())
        .collect::<Vec<_>>();

    let mut cursor = response.cursor;
    while cursor.is_some() {
        let response = list_hosts(client, relay, cursor).await?;
        pdss.extend(response.hosts.into_iter().map(|host| host.into()));
        cursor = response.cursor;
    }

    Ok(pdss)
}

async fn list_hosts(
    client: &reqwest::Client,
    relay: &Relay,
    cursor: Option<String>,
) -> Result<ListHostsResponse, Error> {
    Ok(client
        .get(format!(
            "https://{}/xrpc/com.atproto.sync.listHosts?limit=1000{}",
            relay.host,
            cursor.map(|s| format!("&cursor={s}")).unwrap_or_default()
        ))
        .send()
        .await?
        .error_for_status()?
        .json::<ListHostsResponse>()
        .await?)
}

#[derive(Debug, Deserialize)]
struct ListHostsResponse {
    cursor: Option<String>,
    hosts: Vec<Host>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Host {
    hostname: String,
    seq: u64,
    account_count: u32,
    status: String,
}

impl From<Host> for (String, Pds) {
    fn from(host: Host) -> Self {
        (
            host.hostname,
            Pds {
                relays: Default::default(),
                account_count: host.account_count as usize,
                status: host.status,
            },
        )
    }
}
