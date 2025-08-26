use std::{collections::VecDeque, str::FromStr, sync::OnceLock};

use anyhow::Context;
use tokio::{sync::RwLock, time};
use tracing::error;

static TRACKER: OnceLock<RwLock<MetricsTracker>> = OnceLock::new();

const ONE_MINUTE: time::Duration = time::Duration::from_secs(60);
const ONE_DAY: time::Duration = time::Duration::from_secs(24 * 60 * 60);

pub(crate) async fn monitor(client: reqwest::Client) -> anyhow::Result<()> {
    let _ = TRACKER.set(RwLock::new(MetricsTracker::init(&client).await?));

    let mut interval = time::interval(ONE_MINUTE);
    // We already queried the metrics above, so don't immediately re-query them.
    interval.tick().await;

    loop {
        let now = interval.tick().await;
        match FirehoseCount::fetch(&client).await {
            Err(e) => error!("Failed to fetch firehose metrics: {e}"),
            Ok(data) => {
                if let Some(tracker) = TRACKER.get() {
                    tracker.write().await.accumulate(now, data);
                }
            }
        }
    }
}

/// Returns the average firehose rates per minute, and the duration over which the average
/// is taken.
pub(crate) async fn average_rates_per_min() -> Option<(FirehoseRate, time::Duration)> {
    match TRACKER.get() {
        Some(tracker) => Some(tracker.read().await.average_per_min()),
        _ => None,
    }
}

macro_rules! record_metrics {
    ($metrics:expr, $metric:expr, $count:expr) => (record_metrics!(
        $metrics,
        $metric,
        $count,
        ("atproto_firehose_ops_total" => ops_total),
        ("atproto_firehose_ops_2048" => ops_2048),
        ("atproto_firehose_ops_5leafsync" => ops_5leafsync),
        ("atproto_firehose_ops_anisota" => ops_anisota),
        ("atproto_firehose_ops_atfile" => ops_atfile),
        ("atproto_firehose_ops_bluesky" => ops_bluesky),
        ("atproto_firehose_ops_bluebadge" => ops_bluebadge),
        ("atproto_firehose_ops_bookhive" => ops_bookhive),
        ("atproto_firehose_ops_bookmark" => ops_bookmark),
        ("atproto_firehose_ops_bot_void" => ops_bot_void),
        ("atproto_firehose_ops_cabildoabierto" => ops_cabildoabierto),
        ("atproto_firehose_ops_flashes" => ops_flashes),
        ("atproto_firehose_ops_flushes" => ops_flushes),
        ("atproto_firehose_ops_frontpage" => ops_frontpage),
        ("atproto_firehose_ops_germ" => ops_germ),
        ("atproto_firehose_ops_grain" => ops_grain),
        ("atproto_firehose_ops_gridsky" => ops_gridsky),
        ("atproto_firehose_ops_leaflet" => ops_leaflet),
        ("atproto_firehose_ops_linkat" => ops_linkat),
        ("atproto_firehose_ops_picosky" => ops_picosky),
        ("atproto_firehose_ops_pinksky" => ops_pinksky),
        ("atproto_firehose_ops_popsky" => ops_popsky),
        ("atproto_firehose_ops_protoscript" => ops_protoscript),
        ("atproto_firehose_ops_rocksky" => ops_rocksky),
        ("atproto_firehose_ops_roomy" => ops_roomy),
        ("atproto_firehose_ops_scrapboard" => ops_scrapboard),
        ("atproto_firehose_ops_shadowsky" => ops_shadowsky),
        ("atproto_firehose_ops_skyblur" => ops_skyblur),
        ("atproto_firehose_ops_skyrdle" => ops_skyrdle),
        ("atproto_firehose_ops_skyspace" => ops_skyspace),
        ("atproto_firehose_ops_slices" => ops_slices),
        ("atproto_firehose_ops_smokesignal" => ops_smokesignal),
        ("atproto_firehose_ops_sonasky" => ops_sonasky),
        ("atproto_firehose_ops_spark" => ops_spark),
        ("atproto_firehose_ops_statusphere" => ops_statusphere),
        ("atproto_firehose_ops_streamplace" => ops_streamplace),
        ("atproto_firehose_ops_tangled" => ops_tangled),
        ("atproto_firehose_ops_tealfm" => ops_tealfm),
        ("atproto_firehose_ops_whitewind" => ops_whitewind),
        ("atproto_firehose_ops_yoten" => ops_yoten)
    ));
    ($metrics:expr, $metric:expr, $count:expr, $(($known_metric:literal => $name:ident)),+) => {
        match $metric {
            $(
                $known_metric => $metrics.$name = $count,
            )+
            _ => (),
        }
    };
}

macro_rules! delta {
    ($current:expr, $last:expr) => (delta!(
        $current,
        $last,
        ops_total,
        ops_2048,
        ops_5leafsync,
        ops_anisota,
        ops_atfile,
        ops_bluesky,
        ops_bluebadge,
        ops_bookhive,
        ops_bookmark,
        ops_bot_void,
        ops_cabildoabierto,
        ops_flashes,
        ops_flushes,
        ops_frontpage,
        ops_germ,
        ops_grain,
        ops_gridsky,
        ops_leaflet,
        ops_linkat,
        ops_picosky,
        ops_pinksky,
        ops_popsky,
        ops_protoscript,
        ops_rocksky,
        ops_roomy,
        ops_scrapboard,
        ops_shadowsky,
        ops_skyblur,
        ops_skyrdle,
        ops_skyspace,
        ops_slices,
        ops_smokesignal,
        ops_sonasky,
        ops_spark,
        ops_statusphere,
        ops_streamplace,
        ops_tangled,
        ops_tealfm,
        ops_whitewind,
        ops_yoten
    ));
    ($current:expr, $last:expr, $($name:ident),+) => {
        FirehoseCount {
            $(
                $name: $current.$name - $last.$name,
            )+
        }
    };
}

macro_rules! accumulate {
    ($acc:expr, $item:expr) => (accumulate!(
        $acc,
        $item,
        ops_total,
        ops_2048,
        ops_5leafsync,
        ops_anisota,
        ops_atfile,
        ops_bluesky,
        ops_bluebadge,
        ops_bookhive,
        ops_bookmark,
        ops_bot_void,
        ops_cabildoabierto,
        ops_flashes,
        ops_flushes,
        ops_frontpage,
        ops_germ,
        ops_grain,
        ops_gridsky,
        ops_leaflet,
        ops_linkat,
        ops_picosky,
        ops_pinksky,
        ops_popsky,
        ops_protoscript,
        ops_rocksky,
        ops_roomy,
        ops_scrapboard,
        ops_shadowsky,
        ops_skyblur,
        ops_skyrdle,
        ops_skyspace,
        ops_slices,
        ops_smokesignal,
        ops_sonasky,
        ops_spark,
        ops_statusphere,
        ops_streamplace,
        ops_tangled,
        ops_tealfm,
        ops_whitewind,
        ops_yoten
    ));
    ($acc:expr, $item:expr, $($name:ident),+) => {
        $(
            $acc.$name += $item.$name;
        )+
    };
}

macro_rules! rate {
    ($day_sum:expr, $day_count:expr) => (rate!(
        $day_sum,
        $day_count,
        ops_total,
        ops_2048,
        ops_5leafsync,
        ops_anisota,
        ops_atfile,
        ops_bluesky,
        ops_bluebadge,
        ops_bookhive,
        ops_bookmark,
        ops_bot_void,
        ops_cabildoabierto,
        ops_flashes,
        ops_flushes,
        ops_frontpage,
        ops_germ,
        ops_grain,
        ops_gridsky,
        ops_leaflet,
        ops_linkat,
        ops_picosky,
        ops_pinksky,
        ops_popsky,
        ops_protoscript,
        ops_rocksky,
        ops_roomy,
        ops_scrapboard,
        ops_shadowsky,
        ops_skyblur,
        ops_skyrdle,
        ops_skyspace,
        ops_slices,
        ops_smokesignal,
        ops_sonasky,
        ops_spark,
        ops_statusphere,
        ops_streamplace,
        ops_tangled,
        ops_tealfm,
        ops_whitewind,
        ops_yoten
    ));
    ($day_sum:expr, $day_count:expr, $($name:ident),+) => {
        FirehoseRate {
            $(
                $name: $day_sum.$name as f64 / $day_count,
            )+
        }
    };
}

/// Tracks firehose metrics across the past 24 hours.
#[derive(Debug)]
struct MetricsTracker {
    last_count: FirehoseCount,
    day_changes: VecDeque<(time::Instant, FirehoseCount)>,
}

impl MetricsTracker {
    async fn init(client: &reqwest::Client) -> anyhow::Result<Self> {
        let data = FirehoseCount::fetch(&client)
            .await
            .with_context(|| "Failed to fetch firehose metrics")?;

        Ok(Self {
            last_count: data,
            // One extra capacity so we can push and then trim.
            day_changes: VecDeque::new(),
        })
    }

    fn accumulate(&mut self, now: time::Instant, data: FirehoseCount) {
        // If the newly-fetched total is smaller than the previous total, the data source
        // has reset; we skip aggregating this delta and save the new count for the next.
        if data.ops_total >= self.last_count.ops_total {
            let latest_delta = delta!(data, self.last_count);
            self.day_changes.push_front((now, latest_delta));
        }

        while self
            .day_changes
            .back()
            .map(|(t, _)| now - *t > ONE_DAY)
            .unwrap_or(false)
        {
            self.day_changes.pop_back();
        }

        self.last_count = data;
    }

    fn average_per_min(&self) -> (FirehoseRate, time::Duration) {
        let day_sum = self
            .day_changes
            .iter()
            .map(|(_, c)| c)
            .copied()
            .reduce(|mut acc, item| {
                accumulate!(acc, item);
                acc
            })
            .unwrap_or_default();
        let day_count = self.day_changes.len() as f64;

        // The data range is at most 24 hours by construction, but may be fewer.
        let day_range = self
            .day_changes
            .front()
            .zip(self.day_changes.back())
            .map(|((latest, _), (earliest, _))| *latest - *earliest)
            .unwrap_or(time::Duration::ZERO);

        (rate!(day_sum, day_count), day_range)
    }
}

/// Number of operations per minute being emitted from the firehose.
#[derive(Clone, Debug)]
pub(crate) struct FirehoseRate {
    pub(crate) ops_total: f64,
    pub(crate) ops_2048: f64,
    pub(crate) ops_5leafsync: f64,
    pub(crate) ops_anisota: f64,
    pub(crate) ops_atfile: f64,
    pub(crate) ops_bluesky: f64,
    pub(crate) ops_bluebadge: f64,
    pub(crate) ops_bookhive: f64,
    pub(crate) ops_bookmark: f64,
    pub(crate) ops_bot_void: f64,
    pub(crate) ops_cabildoabierto: f64,
    pub(crate) ops_flashes: f64,
    pub(crate) ops_flushes: f64,
    pub(crate) ops_frontpage: f64,
    pub(crate) ops_germ: f64,
    pub(crate) ops_grain: f64,
    pub(crate) ops_gridsky: f64,
    pub(crate) ops_leaflet: f64,
    pub(crate) ops_linkat: f64,
    pub(crate) ops_picosky: f64,
    pub(crate) ops_pinksky: f64,
    pub(crate) ops_popsky: f64,
    pub(crate) ops_protoscript: f64,
    pub(crate) ops_rocksky: f64,
    pub(crate) ops_roomy: f64,
    pub(crate) ops_scrapboard: f64,
    pub(crate) ops_shadowsky: f64,
    pub(crate) ops_skyblur: f64,
    pub(crate) ops_skyrdle: f64,
    pub(crate) ops_skyspace: f64,
    pub(crate) ops_slices: f64,
    pub(crate) ops_smokesignal: f64,
    pub(crate) ops_sonasky: f64,
    pub(crate) ops_spark: f64,
    pub(crate) ops_statusphere: f64,
    pub(crate) ops_streamplace: f64,
    pub(crate) ops_tangled: f64,
    pub(crate) ops_tealfm: f64,
    pub(crate) ops_whitewind: f64,
    pub(crate) ops_yoten: f64,
}

impl FirehoseRate {
    pub(crate) fn groups(&self) -> Vec<FirehoseGroup> {
        let g = |title, uri, rate| FirehoseGroup {
            info: GroupInfo { title, uri },
            rate,
            subgroups: &[],
        };

        let mut groups = vec![
            g("2048 game", "https://2048.blue/", self.ops_2048),
            g("anisota", "https://anisota.net/", self.ops_anisota),
            g(
                "ATFile",
                "https://github.com/electricduck/atfile",
                self.ops_atfile,
            ),
            g("Bluesky", "https://bsky.app/", self.ops_bluesky),
            g("Blue Badge", "https://badge.blue/", self.ops_bluebadge),
            g("BookHive", "https://bookhive.buzz/", self.ops_bookhive),
            FirehoseGroup {
                info: GroupInfo {
                    title: "Bookmarks",
                    uri: "",
                },
                rate: self.ops_bookmark,
                subgroups: &[
                    GroupInfo {
                        title: "Klearsky",
                        uri: "https://github.com/mimonelu/klearsky",
                    },
                    GroupInfo {
                        title: "Starrysky",
                        uri: "https://starrysky-console.pages.dev/",
                    },
                ],
            },
            g(
                "Cabildo Abierto",
                "https://www.cabildoabierto.ar/",
                self.ops_cabildoabierto,
            ),
            g(
                "Flashes",
                "https://bsky.app/profile/flashes.blue",
                self.ops_flashes,
            ),
            g("Flushes", "https://flushes.app/", self.ops_flushes),
            g("Frontpage", "https://frontpage.fyi/", self.ops_frontpage),
            g("Germ", "https://www.germnetwork.com/", self.ops_germ),
            g("Grain", "https://grain.social/", self.ops_grain),
            g("Gridsky", "https://gridsky.app/", self.ops_gridsky),
            g("Leaflet", "https://leaflet.pub/", self.ops_leaflet),
            g("Linkat", "https://linkat.blue/", self.ops_linkat),
            g("Picosky", "https://psky.social/", self.ops_picosky),
            g("Pinksky", "https://pinksky.app/", self.ops_pinksky),
            g("Popsky", "https://popsky.social/", self.ops_popsky),
            g(
                "ProtoScript",
                "https://protoscript.atdev.pro/",
                self.ops_protoscript,
            ),
            g("Rocksky", "https://rocksky.app/", self.ops_rocksky),
            g("Roomy", "https://roomy.chat/", self.ops_roomy),
            g("Scrapboard", "https://scrapboard.org/", self.ops_scrapboard),
            g("ShadowSky", "https://shadowsky.io/", self.ops_shadowsky),
            g("Skyblur", "https://skyblur.uk/", self.ops_skyblur),
            g("Skyrdle", "https://skyrdle.com/", self.ops_skyrdle),
            g("SkySpace", "https://skyspace.me/", self.ops_skyspace),
            g(
                "Slices",
                "https://bsky.app/profile/chadtmiller.com/post/3lxb6wem4622j",
                self.ops_slices,
            ),
            g(
                "Smoke Signal",
                "https://smokesignal.events/",
                self.ops_smokesignal,
            ),
            g("SonaSky", "https://sonasky.app/", self.ops_sonasky),
            g("Spark", "https://sprk.so/", self.ops_spark),
            g(
                "Statusphere",
                "https://atproto.com/guides/applications",
                self.ops_statusphere,
            ),
            g("Streamplace", "https://stream.place/", self.ops_streamplace),
            g("Tangled", "https://tangled.sh/", self.ops_tangled),
            g("teal.fm", "https://teal.fm/", self.ops_tealfm),
            g(
                "Void Bot",
                "https://cameron.pfiffer.org/blog/void/",
                self.ops_bot_void,
            ),
            g("WhiteWind", "https://whtwnd.com/", self.ops_whitewind),
            g("Yōten", "https://yoten.app/", self.ops_yoten),
            g("Some kind of sync from Mastodon", "", self.ops_5leafsync),
        ];

        // Only show groups with observed records over the measurement interval.
        groups.retain(|g| g.rate > 0.0);

        // Sort from highest rate to lowest. This doesn't directly map to activity because
        // different records have different intended usage patterns (and thus a single
        // record being observed in one group could be equivalent to hundreds of records
        // in another group), but it suffices for now.
        groups.sort_by(|a, b| a.rate.total_cmp(&b.rate));
        groups.reverse();

        groups
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct FirehoseCount {
    ops_total: u64,
    ops_2048: u64,
    ops_5leafsync: u64,
    ops_anisota: u64,
    ops_atfile: u64,
    ops_bluesky: u64,
    ops_bluebadge: u64,
    ops_bookhive: u64,
    ops_bookmark: u64,
    ops_bot_void: u64,
    ops_cabildoabierto: u64,
    ops_flashes: u64,
    ops_flushes: u64,
    ops_frontpage: u64,
    ops_germ: u64,
    ops_grain: u64,
    ops_gridsky: u64,
    ops_leaflet: u64,
    ops_linkat: u64,
    ops_picosky: u64,
    ops_pinksky: u64,
    ops_popsky: u64,
    ops_protoscript: u64,
    ops_rocksky: u64,
    ops_roomy: u64,
    ops_scrapboard: u64,
    ops_shadowsky: u64,
    ops_skyblur: u64,
    ops_skyrdle: u64,
    ops_skyspace: u64,
    ops_slices: u64,
    ops_smokesignal: u64,
    ops_sonasky: u64,
    ops_spark: u64,
    ops_statusphere: u64,
    ops_streamplace: u64,
    ops_tangled: u64,
    ops_tealfm: u64,
    ops_whitewind: u64,
    ops_yoten: u64,
}

impl FromStr for FirehoseCount {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut metrics = FirehoseCount::default();
        for line in s.lines() {
            if let Some((metric, value)) = line.split_once(' ') {
                if let Ok(count) = value.parse::<u64>() {
                    record_metrics!(metrics, metric, count);
                }
            }
        }
        Ok(metrics)
    }
}

impl FirehoseCount {
    async fn fetch(client: &reqwest::Client) -> anyhow::Result<Self> {
        let data = client
            .get("http://app.process.str4d-bots.internal:9000")
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?
            .parse::<Self>()?;

        Ok(data)
    }
}

pub(crate) struct FirehoseGroup {
    pub(crate) info: GroupInfo,
    pub(crate) rate: f64,
    pub(crate) subgroups: &'static [GroupInfo],
}

pub(crate) struct GroupInfo {
    pub(crate) title: &'static str,
    pub(crate) uri: &'static str,
}
