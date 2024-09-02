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

pub(crate) async fn average_rates_per_min() -> Option<FirehoseRate> {
    if let Some(tracker) = TRACKER.get() {
        Some(tracker.read().await.average_per_min())
    } else {
        None
    }
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
        let latest_delta = FirehoseCount {
            ops_total: data.ops_total - self.last_count.ops_total,
            ops_bluesky: data.ops_bluesky - self.last_count.ops_bluesky,
            ops_frontpage: data.ops_frontpage - self.last_count.ops_frontpage,
            ops_smokesignal: data.ops_smokesignal - self.last_count.ops_smokesignal,
            ops_whitewind: data.ops_whitewind - self.last_count.ops_whitewind,
        };

        self.day_changes.push_front((now, latest_delta));
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

    fn average_per_min(&self) -> FirehoseRate {
        let day_sum = self
            .day_changes
            .iter()
            .map(|(_, c)| c)
            .copied()
            .reduce(|mut acc, item| {
                acc.ops_total += item.ops_total;
                acc.ops_bluesky += item.ops_bluesky;
                acc.ops_frontpage += item.ops_frontpage;
                acc.ops_smokesignal += item.ops_smokesignal;
                acc.ops_whitewind += item.ops_whitewind;
                acc
            })
            .unwrap_or_default();
        let day_count = self.day_changes.len() as f64;

        FirehoseRate {
            ops_total: day_sum.ops_total as f64 / day_count,
            ops_bluesky: day_sum.ops_bluesky as f64 / day_count,
            ops_frontpage: day_sum.ops_frontpage as f64 / day_count,
            ops_smokesignal: day_sum.ops_smokesignal as f64 / day_count,
            ops_whitewind: day_sum.ops_whitewind as f64 / day_count,
        }
    }
}

/// Number of operations per minute being emitted from the firehose.
#[derive(Clone, Debug)]
pub(crate) struct FirehoseRate {
    pub(crate) ops_total: f64,
    pub(crate) ops_bluesky: f64,
    pub(crate) ops_frontpage: f64,
    pub(crate) ops_smokesignal: f64,
    pub(crate) ops_whitewind: f64,
}

#[derive(Clone, Copy, Debug, Default)]
struct FirehoseCount {
    ops_total: u64,
    ops_bluesky: u64,
    ops_frontpage: u64,
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
                    match metric {
                        "atproto_firehose_ops_total" => metrics.ops_total = count,
                        "atproto_firehose_ops_bluesky" => metrics.ops_bluesky = count,
                        "atproto_firehose_ops_frontpage" => metrics.ops_frontpage = count,
                        "atproto_firehose_ops_smokesignal" => metrics.ops_smokesignal = count,
                        "atproto_firehose_ops_whitewind" => metrics.ops_whitewind = count,
                        _ => (),
                    }
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
