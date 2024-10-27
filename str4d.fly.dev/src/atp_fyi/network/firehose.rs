use std::{collections::VecDeque, str::FromStr, sync::OnceLock};

use anyhow::Context;
use tokio::{sync::RwLock, time};
use tracing::error;

static TRACKER: OnceLock<RwLock<MetricsTracker>> = OnceLock::new();

const ONE_MINUTE: time::Duration = time::Duration::from_secs(60);
const ONE_DAY: time::Duration = time::Duration::from_secs(24 * 60 * 60);

pub(crate) async fn monitor() -> anyhow::Result<()> {
    let client = reqwest::Client::builder().build()?;
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
    if let Some(tracker) = TRACKER.get() {
        Some(tracker.read().await.average_per_min())
    } else {
        None
    }
}

macro_rules! record_metrics {
    ($metrics:expr, $metric:expr, $count:expr) => (record_metrics!(
        $metrics,
        $metric,
        $count,
        ("atproto_firehose_ops_total" => ops_total),
        ("atproto_firehose_ops_bluesky" => ops_bluesky),
        ("atproto_firehose_ops_frontpage" => ops_frontpage),
        ("atproto_firehose_ops_picosky" => ops_picosky),
        ("atproto_firehose_ops_smokesignal" => ops_smokesignal),
        ("atproto_firehose_ops_whitewind" => ops_whitewind)
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
        ops_bluesky,
        ops_frontpage,
        ops_picosky,
        ops_smokesignal,
        ops_whitewind
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
        ops_bluesky,
        ops_frontpage,
        ops_picosky,
        ops_smokesignal,
        ops_whitewind
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
        ops_bluesky,
        ops_frontpage,
        ops_picosky,
        ops_smokesignal,
        ops_whitewind
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
    pub(crate) ops_bluesky: f64,
    pub(crate) ops_frontpage: f64,
    pub(crate) ops_picosky: f64,
    pub(crate) ops_smokesignal: f64,
    pub(crate) ops_whitewind: f64,
}

#[derive(Clone, Copy, Debug, Default)]
struct FirehoseCount {
    ops_total: u64,
    ops_bluesky: u64,
    ops_frontpage: u64,
    ops_picosky: u64,
    ops_smokesignal: u64,
    ops_whitewind: u64,
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
