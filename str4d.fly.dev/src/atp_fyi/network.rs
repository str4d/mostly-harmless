use std::collections::BTreeMap;

use atrium_api::{
    app::bsky::{feed, labeler},
    xrpc,
};
use serde::Serialize;

pub(crate) mod firehose;
mod services;

const NODE_MIN_AREA: f64 = 4.0;
const NODE_MAX_AREA: f64 = 225.0;

const EDGE_MIN_SIZE: f64 = 1.0;
const EDGE_MAX_SIZE: f64 = 10.0;

pub(super) async fn render_map(client: &reqwest::Client) -> Result<Map, Error> {
    let network = services::enumerate(client).await?;
    let rates = firehose::average_rates_per_min()
        .await
        .map(|(rates, _)| rates)
        // Use canned data if we're testing locally.
        .unwrap_or(firehose::FirehoseRate {
            ops_total: 26507.0,
            ops_2048: 0.091,
            ops_5leafsync: 1.951,
            ops_anisota: 0.700,
            ops_atfile: 0.001,
            ops_blatball: 4.000,
            ops_blento: 0.124,
            ops_bluebadge: 0.000,
            ops_bluesky: 26507.0,
            ops_bookhive: 0.026,
            ops_bookmark: 0.000,
            ops_bot_void: 1.287,
            ops_cabildoabierto: 0.006,
            ops_deckbelcher: 0.511,
            ops_flashes: 0.348,
            ops_flushes: 0.005,
            ops_frontpage: 0.002,
            ops_germ: 0.021,
            ops_grain: 0.222,
            ops_gridsky: 0.000,
            ops_leaflet: 0.008,
            ops_linkat: 0.003,
            ops_margin: 0.082,
            ops_picosky: 0.000,
            ops_pinksky: 0.061,
            ops_podping: 13.886,
            ops_popfeed: 0.308,
            ops_popsky: 0.000,
            ops_protoscript: 0.000,
            ops_rocksky: 2.639,
            ops_roomy: 0.000,
            ops_scrapboard: 0.000,
            ops_semble: 0.176,
            ops_shadowsky: 0.000,
            ops_sigintteam: 5.981,
            ops_skyblur: 0.056,
            ops_skyrdle: 0.017,
            ops_skyspace: 0.026,
            ops_slices: 0.000,
            ops_smokesignal: 0.001,
            ops_sonasky: 0.000,
            ops_spark: 0.129,
            ops_standardsite: 0.459,
            ops_statusphere: 0.008,
            ops_streamplace: 10.765,
            ops_tangled: 0.245,
            ops_tealfm: 22.043,
            ops_whitewind: 0.031,
            ops_yoten: 0.000,
        });

    let node_builder = NodeBuilder::new(&rates, &network);
    let edge_builder = EdgeBuilder::new(&rates);

    let mut nodes = Vec::with_capacity(
        network.relays.len()
            + network.pdss.len()
            + network.labelers.len()
            + network.feeds.len()
            // AppViews (see below)
            + 24,
    );

    let mut add_node = |node| {
        let index = nodes.len();
        nodes.push(node);
        index
    };

    // Add nodes for all detected PDSs.
    let pds_nodes = network
        .pdss
        .into_iter()
        .map(|(hostname, pds)| {
            (
                add_node(node_builder.pds(hostname.clone(), pds.account_count)),
                pds.account_count,
                pds.relays,
            )
        })
        .collect::<Vec<_>>();

    // Add nodes for the known relays.
    let relay_nodes = network
        .relays
        .into_iter()
        .enumerate()
        .map(|(relay_index, relay)| {
            let accounts = pds_nodes
                .iter()
                .filter_map(|(_, account_count, relays)| {
                    relays.contains(&relay_index).then_some(*account_count)
                })
                .sum::<usize>();
            (
                relay_index,
                add_node(node_builder.relay(
                    relay,
                    // Approximate relay rate by how many accounts it is receiving events from.
                    rates.ops_total * (accounts as f64) / (node_builder.total_pds_accounts as f64),
                )),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let bsky_relays = [0, 1].map(|relay_index| *relay_nodes.get(&relay_index).expect("present"));
    let blacksky_relay = *relay_nodes.get(&2).expect("present");

    // Add all detected labelers.
    for labeler in network.labelers {
        add_node(node_builder.labeler(labeler));
    }

    // Add all detected feeds.
    for feed in network.feeds {
        add_node(node_builder.feed(feed));
    }

    // Add the known appviews.
    // Hard-coded for now.
    let appview_bsky = add_node(node_builder.app_view("Bluesky".into(), rates.ops_bluesky));
    let appview_blacksky = add_node(node_builder.app_view("Blacksky".into(), rates.ops_bluesky));
    let appview_anisota = add_node(node_builder.app_view("anisota".into(), rates.ops_anisota));
    let appview_blatball = add_node(node_builder.app_view("Blatball".into(), rates.ops_blatball));
    let appview_blento = add_node(node_builder.app_view("blento".into(), rates.ops_blento));
    let appview_deckbelcher =
        add_node(node_builder.app_view("deck belcher".into(), rates.ops_deckbelcher));
    let appview_flashes = add_node(node_builder.app_view("Flashes".into(), rates.ops_flashes));
    let appview_frontpage =
        add_node(node_builder.app_view("Frontpage".into(), rates.ops_frontpage));
    let appview_grain = add_node(node_builder.app_view("Grain".into(), rates.ops_grain));
    let appview_leaflet = add_node(
        node_builder.app_view("Leaflet".into(), rates.ops_standardsite + rates.ops_leaflet),
    );
    let appview_linkat = add_node(node_builder.app_view("Linkat".into(), rates.ops_linkat));
    let appview_picosky = add_node(node_builder.app_view("Picosky".into(), rates.ops_picosky));
    let appview_pinksky = add_node(node_builder.app_view("Pinksky".into(), rates.ops_pinksky));
    let appview_popsky = add_node(node_builder.app_view("Popsky".into(), rates.ops_popsky));
    let appview_rocksky = add_node(node_builder.app_view("Rocksky".into(), rates.ops_rocksky));
    let appview_roomy = add_node(node_builder.app_view("Roomy".into(), rates.ops_roomy));
    let appview_semble = add_node(node_builder.app_view("Semble".into(), rates.ops_semble));
    let appview_skyspace = add_node(node_builder.app_view("SkySpace".into(), rates.ops_skyspace));
    let appview_smokesignal =
        add_node(node_builder.app_view("Smoke Signal".into(), rates.ops_smokesignal));
    let appview_sonasky = add_node(node_builder.app_view("SonaSky".into(), rates.ops_sonasky));
    let appview_streamplace =
        add_node(node_builder.app_view("Streamplace".into(), rates.ops_streamplace));
    let appview_tangled = add_node(node_builder.app_view("Tangled".into(), rates.ops_tangled));
    let appview_tealfm = add_node(node_builder.app_view("teal.fm".into(), rates.ops_tealfm));
    let appview_whitewind =
        add_node(node_builder.app_view("White Wind".into(), rates.ops_whitewind));

    let mut edges = vec![];

    // Add edges from every PDS to the relays they are connected to.
    for (pds, _, relays) in pds_nodes {
        edges.extend(relays.into_iter().map(|relay_index| {
            edge_builder.pds_to_relay(pds, *relay_nodes.get(&relay_index).expect("present"))
        }));
    }

    // Add edges from the relay to every labeler and feed. We do this first so they render
    // below edges from labelers to appviews.
    edges.extend(
        nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| matches!(n.group, Group::Feed | Group::Labeler))
            .flat_map(|(node, n)| {
                if matches!(n.group, Group::Labeler) {
                    if n.label == "Blacksky Moderation" {
                        [
                            Some(edge_builder.relay_to_labeler(
                                blacksky_relay,
                                node,
                                rates.ops_total,
                            )),
                            None,
                        ]
                    } else {
                        bsky_relays.map(|relay| {
                            Some(edge_builder.relay_to_labeler(relay, node, rates.ops_total))
                        })
                    }
                } else if n.label == "Blacksky" {
                    [
                        Some(edge_builder.relay_to_feed(blacksky_relay, node, rates.ops_total)),
                        None,
                    ]
                } else {
                    bsky_relays
                        .map(|relay| Some(edge_builder.relay_to_feed(relay, node, rates.ops_total)))
                }
            })
            .flatten(),
    );

    // Add edges from every labeler to the appviews that hydrate from them.
    edges.extend(
        nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| matches!(n.group, Group::Labeler))
            .flat_map(|(labeler, _)| {
                [
                    edge_builder.labeler_to_app_view(labeler, appview_bsky),
                    edge_builder.labeler_to_app_view(labeler, appview_blacksky),
                ]
            }),
    );

    // Add edges from the relay to the appviews.
    // TODO: Detect which relays an appview is subscribed to.
    edges.extend(bsky_relays.into_iter().flat_map(|relay| {
        [
            edge_builder.relay_to_app_view(relay, appview_bsky, rates.ops_bluesky),
            edge_builder.relay_to_app_view(relay, appview_anisota, rates.ops_anisota),
            edge_builder.relay_to_app_view(relay, appview_blatball, rates.ops_blatball),
            edge_builder.relay_to_app_view(relay, appview_blento, rates.ops_blento),
            edge_builder.relay_to_app_view(relay, appview_deckbelcher, rates.ops_deckbelcher),
            edge_builder.relay_to_app_view(relay, appview_flashes, rates.ops_flashes),
            edge_builder.relay_to_app_view(relay, appview_frontpage, rates.ops_frontpage),
            edge_builder.relay_to_app_view(relay, appview_grain, rates.ops_grain),
            edge_builder.relay_to_app_view(
                relay,
                appview_leaflet,
                rates.ops_standardsite + rates.ops_leaflet,
            ),
            edge_builder.relay_to_app_view(relay, appview_linkat, rates.ops_linkat),
            edge_builder.relay_to_app_view(relay, appview_picosky, rates.ops_picosky),
            edge_builder.relay_to_app_view(relay, appview_pinksky, rates.ops_pinksky),
            edge_builder.relay_to_app_view(relay, appview_popsky, rates.ops_popsky),
            edge_builder.relay_to_app_view(relay, appview_rocksky, rates.ops_rocksky),
            edge_builder.relay_to_app_view(relay, appview_roomy, rates.ops_roomy),
            edge_builder.relay_to_app_view(relay, appview_semble, rates.ops_semble),
            edge_builder.relay_to_app_view(relay, appview_skyspace, rates.ops_skyspace),
            edge_builder.relay_to_app_view(relay, appview_smokesignal, rates.ops_smokesignal),
            edge_builder.relay_to_app_view(relay, appview_sonasky, rates.ops_sonasky),
            edge_builder.relay_to_app_view(relay, appview_streamplace, rates.ops_streamplace),
            edge_builder.relay_to_app_view(relay, appview_tangled, rates.ops_tangled),
            edge_builder.relay_to_app_view(relay, appview_tealfm, rates.ops_tealfm),
            edge_builder.relay_to_app_view(relay, appview_whitewind, rates.ops_whitewind),
        ]
    }));
    edges.push(edge_builder.relay_to_app_view(blacksky_relay, appview_blacksky, rates.ops_bluesky));

    Ok(Map { nodes, edges })
}

struct LogishNodeScale {
    area_scale: f64,
    floor_offset: f64,
}

struct NodeScale {
    logish: Vec<(f64, LogishNodeScale)>,
    log: LogishNodeScale,
}

impl NodeScale {
    fn new(lin_min_val: f64, log_min_val: f64, max_val: f64) -> Self {
        // Approximate 1 - sqrt(x/5) to get smoothish area scaling.
        let logish = [1.0, 0.55, 0.37, 0.23, 0.11]
            .into_iter()
            .map(|n| {
                let min_val_n = lin_min_val.powf(n);
                let area_scale = (NODE_MAX_AREA - NODE_MIN_AREA) / (max_val.powf(n) / min_val_n);
                let floor_offset = NODE_MIN_AREA - area_scale * min_val_n;

                (
                    n,
                    LogishNodeScale {
                        area_scale,
                        floor_offset,
                    },
                )
            })
            .collect();

        let log_area_scale = (NODE_MAX_AREA - NODE_MIN_AREA) / (max_val / log_min_val).log2();
        let log_floor_offset = NODE_MIN_AREA - log_area_scale * log_min_val.log2();

        Self {
            logish,
            log: LogishNodeScale {
                area_scale: log_area_scale,
                floor_offset: log_floor_offset,
            },
        }
    }

    fn radius(&self, value: f64) -> Vec<f64> {
        // We want to scale the node area by value. Sigma.js only has a radius control, so
        // we convert from area to radius afterwards.
        self.logish
            .iter()
            .map(|(n, scale)| {
                let area = scale.floor_offset + scale.area_scale * value.powf(*n);
                area.sqrt()
            })
            .chain(Some(self.log_radius(value)))
            .collect()
    }

    fn log_radius(&self, value: f64) -> f64 {
        // We want to scale the node area by value. Sigma.js only has a radius control, so
        // we convert from area to radius afterwards.
        let area = self.log.floor_offset + self.log.area_scale * value.log2();
        area.sqrt()
    }
}

struct NodeBuilder {
    pds_scale: NodeScale,
    relay_scale: NodeScale,
    labeler_scale: NodeScale,
    feed_scale: NodeScale,
    app_view_scale: NodeScale,
    total_pds_accounts: usize,
}

impl NodeBuilder {
    fn new(rates: &firehose::FirehoseRate, network: &services::Network) -> Self {
        // Scale groups based on "equivalent total throughput":
        // - All PDS users contribute towards all events being emitted from the relay.
        // - All labels reach all AppViews.
        // - All users who have authored posts contribute to AppViews.
        let total_pds_users: usize = network.pdss.iter().map(|(_, pds)| pds.account_count).sum();
        let largest_pds_users: usize = network
            .pdss
            .iter()
            .map(|(_, pds)| pds.account_count)
            .max()
            .unwrap_or(total_pds_users);
        let max_relay_rate = rates.ops_total;
        let max_labeler_likes = network
            .labelers
            .iter()
            .map(|labeler| labeler.likes)
            .max()
            .unwrap_or(1);
        let min_feed_likes = network
            .feeds
            .iter()
            .map(|feed| feed.likes)
            .min()
            .unwrap_or(1);
        let max_feed_likes = network
            .feeds
            .iter()
            .map(|feed| feed.likes)
            .max()
            .unwrap_or(1);

        Self {
            pds_scale: NodeScale::new(1.0, 1.0, largest_pds_users as f64),
            relay_scale: NodeScale::new(0.01, 0.01, max_relay_rate),
            labeler_scale: NodeScale::new(1.0, 1.0, max_labeler_likes as f64),
            feed_scale: NodeScale::new(1.0, min_feed_likes as f64, max_feed_likes as f64),
            app_view_scale: NodeScale::new(1.0, 1.0, max_relay_rate as f64),
            total_pds_accounts: total_pds_users,
        }
    }

    fn make_node(
        &self,
        group: Group,
        subgroup: String,
        label: String,
        value: f64,
        bsky_operated: bool,
    ) -> Node {
        let scale = match group {
            Group::Pds => &self.pds_scale,
            Group::Relay => &self.relay_scale,
            Group::Labeler => &self.labeler_scale,
            Group::Feed => &self.feed_scale,
            Group::AppView => &self.app_view_scale,
        };

        Node {
            group,
            subgroup,
            label,
            logish_sizes: scale.radius(value),
            bsky_operated,
        }
    }

    fn pds(&self, hostname: String, users: usize) -> Node {
        let bsky_operated = hostname.ends_with(".bsky.network");
        self.make_node(
            Group::Pds,
            String::new(),
            format!(
                "{hostname} ({})",
                if users == 1 {
                    "1 account".into()
                } else {
                    format!("{users} accounts")
                }
            ),
            users as f64,
            bsky_operated,
        )
    }

    fn relay(&self, relay: services::Relay, ops_per_minute: f64) -> Node {
        self.make_node(
            Group::Relay,
            relay.region.to_string(),
            relay.name.to_string(),
            ops_per_minute,
            relay.bsky_operated,
        )
    }

    fn labeler(&self, labeler: services::Labeler) -> Node {
        self.make_node(
            Group::Labeler,
            String::new(),
            labeler.name,
            labeler.likes as f64,
            labeler.bsky_operated,
        )
    }

    fn feed(&self, feed: services::Feed) -> Node {
        self.make_node(
            Group::Feed,
            String::new(),
            feed.name,
            feed.likes as f64,
            feed.bsky_operated,
        )
    }

    fn app_view(&self, label: String, ops_per_minute: f64) -> Node {
        let bsky_operated = label == "Bluesky";
        // TODO: This will not stay as ops_per_minute, because that's more of an edge
        // metric, and for some kinds of AppViews (like White Wind) usage might be high
        // even if ops/min is low.
        self.make_node(
            Group::AppView,
            String::new(),
            label,
            ops_per_minute,
            bsky_operated,
        )
    }
}

struct EdgeBuilder {
    relay_scale: f64,
}

impl EdgeBuilder {
    fn new(rates: &firehose::FirehoseRate) -> Self {
        let relay_scale = (EDGE_MAX_SIZE - EDGE_MIN_SIZE) / rates.ops_total;

        Self { relay_scale }
    }

    fn pds_to_relay(&self, from: usize, to: usize) -> Edge {
        Edge {
            from,
            to,
            size: EDGE_MIN_SIZE,
            colour: "#ababff",
        }
    }

    fn relay_to_labeler(&self, from: usize, to: usize, ops: f64) -> Edge {
        Edge {
            from,
            to,
            size: EDGE_MIN_SIZE.max(ops * self.relay_scale),
            colour: "#ffd17a",
        }
    }

    fn relay_to_feed(&self, from: usize, to: usize, ops: f64) -> Edge {
        Edge {
            from,
            to,
            size: EDGE_MIN_SIZE.max(ops * self.relay_scale),
            colour: "#ffd17a",
        }
    }

    fn labeler_to_app_view(&self, from: usize, to: usize) -> Edge {
        Edge {
            from,
            to,
            size: EDGE_MIN_SIZE,
            colour: "#ff8f8f",
        }
    }

    fn relay_to_app_view(&self, from: usize, to: usize, ops: f64) -> Edge {
        Edge {
            from,
            to,
            size: EDGE_MIN_SIZE.max(ops * self.relay_scale),
            colour: "#ffd17a",
        }
    }
}

#[derive(Clone, Serialize)]
pub(super) struct Map {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

#[derive(Clone, Serialize)]
struct Node {
    group: Group,
    subgroup: String,
    label: String,
    /// First entry is linear scale, last entry is log scale.
    logish_sizes: Vec<f64>,
    bsky_operated: bool,
}

#[derive(Clone, Serialize)]
enum Group {
    Pds,
    Relay,
    Labeler,
    Feed,
    AppView,
}

#[derive(Clone, Serialize)]
struct Edge {
    from: usize,
    to: usize,
    size: f64,
    colour: &'static str,
}

#[derive(Debug)]
#[allow(dead_code)]
pub(super) enum Error {
    BlueskyAuthRequired,
    Feed(xrpc::Error<feed::get_suggested_feeds::Error>),
    Http(reqwest::Error),
    Labeler(xrpc::Error<labeler::get_services::Error>),
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Http(e)
    }
}

impl From<xrpc::Error<feed::get_suggested_feeds::Error>> for Error {
    fn from(e: xrpc::Error<feed::get_suggested_feeds::Error>) -> Self {
        Error::Feed(e)
    }
}

impl From<xrpc::Error<labeler::get_services::Error>> for Error {
    fn from(e: xrpc::Error<labeler::get_services::Error>) -> Self {
        Error::Labeler(e)
    }
}
