use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;

use crate::rfc_observer::common::{Bucket, HistogramStats, completion_months_histogram};

use super::datatracker::DocInfo;

#[derive(Clone, Debug, Serialize)]
pub(super) struct Document {
    name: String,
    title: String,
    pub(super) rfc: Option<u32>,
    pub(super) created_at: DateTime<Utc>,
    pub(super) revisions: Vec<DateTime<Utc>>,
    pub(super) expires_at: Option<DateTime<Utc>>,
    pub(super) closed_at: Option<DateTime<Utc>>,
}

impl Document {
    pub(super) fn new(
        rfc: Option<u32>,
        info: DocInfo,
        expires_at: Option<DateTime<Utc>>,
    ) -> Option<Self> {
        let mut rev_history = info.rev_history;
        let creation = rev_history.remove(0);
        let closed_at = rfc.map(|_| rev_history.pop().unwrap().published);

        Some(Self {
            name: info.name,
            title: info.title,
            rfc,
            created_at: creation.published,
            revisions: rev_history.into_iter().map(|rev| rev.published).collect(),
            expires_at,
            closed_at,
        })
    }
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct Aggregate {
    date: NaiveDate,
    draft: u64,
    expired: u64,
    published: u64,
}

#[derive(Clone, Serialize)]
pub(super) struct Data {
    pub(super) agg: Vec<Aggregate>,
    pub(super) completed_hist: Vec<Bucket>,
    pub(super) completed_stats: HistogramStats,
    pub(super) open: Vec<Document>,
    pub(super) closed: Vec<Document>,
}

impl Data {
    pub(super) fn new(documents: Vec<Document>) -> Self {
        let now = Utc::now();

        // First, build up a map of deltas for each day we have a publication event on.
        #[derive(Debug, Default)]
        struct Deltas {
            drafted: i64,
            expired: i64,
            published: i64,
        }
        let mut deltas = BTreeMap::<_, Deltas>::new();
        fn day<'d>(
            deltas: &'d mut BTreeMap<NaiveDate, Deltas>,
            at: &DateTime<Utc>,
        ) -> &'d mut Deltas {
            deltas.entry(at.date_naive()).or_default()
        }

        for doc in &documents {
            day(&mut deltas, &doc.created_at).drafted += 1;

            // Mark each revision as a day without changes, so the transition from last
            // draft to published RFC renders correctly.
            for at in &doc.revisions {
                let _ = day(&mut deltas, &at);
            }

            match (doc.expires_at, doc.closed_at) {
                (Some(at), _) if at <= now => {
                    let d = day(&mut deltas, &at);
                    d.drafted -= 1;
                    d.expired += 1;
                }
                (_, Some(at)) => {
                    let d = day(&mut deltas, &at);
                    d.drafted -= 1;
                    d.published += 1;
                }
                (_, _) => (),
            }
        }

        // Then, create a running sum of the deltas to get the category counts per day.
        let agg = deltas
            .into_iter()
            .scan(None, |state, (date, deltas)| {
                match state {
                    None => {
                        *state = Some(Aggregate {
                            date,
                            draft: deltas.drafted.try_into().expect("first day"),
                            expired: deltas.expired.try_into().expect("first day"),
                            published: deltas.published.try_into().expect("first day"),
                        })
                    }
                    Some(d) => {
                        d.date = date;
                        d.draft = d.draft.checked_add_signed(deltas.drafted).expect("fine");
                        d.expired = d.expired.checked_add_signed(deltas.expired).expect("fine");
                        d.published = d
                            .published
                            .checked_add_signed(deltas.published)
                            .expect("fine");
                    }
                }

                state.clone()
            })
            .collect();

        let mut open = documents
            .iter()
            .filter(|doc| doc.closed_at.is_none() && doc.expires_at.map_or(true, |at| at > now))
            .cloned()
            .collect::<Vec<_>>();

        let mut closed = documents
            .into_iter()
            .filter(|doc| doc.closed_at.is_some())
            .collect::<Vec<_>>();

        // Sort open docs by length of time they have been open.
        open.sort_by_cached_key(|doc| now - doc.created_at);
        open.reverse();

        // Sort closed docs by length of time they were open.
        closed.sort_by_cached_key(|doc| doc.closed_at.unwrap() - doc.created_at);
        closed.reverse();

        // Prepare a histogram of "number of RFCs completed within X months".
        let (completed_hist, completed_stats) = completion_months_histogram(&closed, |doc| {
            (&doc.created_at, &doc.closed_at.as_ref().unwrap())
        });

        Self {
            agg,
            completed_hist,
            completed_stats,
            open,
            closed,
        }
    }
}
