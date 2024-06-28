use std::{cell::OnceCell, collections::BTreeMap, str::FromStr};

use chrono::{DateTime, Utc};
use phf::phf_map;
use regex::Regex;
use serde::Serialize;
use tracing::debug;

use crate::rfc_observer::common::{
    issues_with_labels_query::IssuesWithLabelsQueryRepositoryIssuesEdgesNode, label_events_for,
    LabelEvent,
};

/// Issues that get detected as RFC tracking issues, but that should be ignored (because
/// e.g. they are a duplicate).
const ISSUES_TO_IGNORE: &[i64] = &[14812];

/// Manual mapping of tracking issues to RFCs, for the few cases where we can't extract
/// the RFC number automatically.
static RFC_FOR_ISSUE: phf::Map<i64, u32> = phf_map! {
    16461_i64 => 47,
    17841_i64 => 195, // (subtask)
    19794_i64 => 439, // (more targeted issue)
    19795_i64 => 439, // (more targeted issue)
    23086_i64 => 1023,
    23416_i64 => 803, // First URL in top comment is for a later RFC.
    23533_i64 => 940,
    44752_i64 => 2115, // (subtask)
    55913_i64 => 911, // (more targeted issue)
    91399_i64 => 3173,
};

const RE_RFC_PR: OnceCell<Regex> = OnceCell::new();
const RE_RFC_TEXT: OnceCell<Regex> = OnceCell::new();
const RE_RFC_RENDERED: OnceCell<Regex> = OnceCell::new();
const RE_RFC_TITLE: OnceCell<Regex> = OnceCell::new();

#[derive(Clone, Debug, Serialize)]
pub(super) struct TrackingIssue {
    number: i64,
    title: String,
    pub(super) rfc: u32,
    pub(super) created_at: DateTime<Utc>,
    pub(super) approved_at: DateTime<Utc>,
    pub(super) implemented_at: Option<DateTime<Utc>>,
    pub(super) closed_at: Option<DateTime<Utc>>,
}

impl TrackingIssue {
    pub(super) fn new(issue: IssuesWithLabelsQueryRepositoryIssuesEdgesNode) -> Option<Self> {
        if ISSUES_TO_IGNORE.contains(&issue.number) {
            return None;
        }

        // Attempt to identify the RFC.
        let rfc = match RFC_FOR_ISSUE.get(&issue.number) {
            Some(rfc) => *rfc,
            None => match RE_RFC_PR
                .get_or_init(|| Regex::new(r"rust-lang\/rfcs(\/pull\/|#)(\d+)").unwrap())
                .captures(&issue.body)
                .and_then(|c| c.get(2))
                .or_else(|| {
                    RE_RFC_TEXT
                        .get_or_init(|| {
                            Regex::new(r"rust-lang\/rfcs\/blob\/master\/text\/(\d+)-").unwrap()
                        })
                        .captures(&issue.body)
                        .and_then(|c| c.get(1))
                })
                .or_else(|| {
                    RE_RFC_RENDERED
                        .get_or_init(|| Regex::new(r"rust-lang.github.io\/rfcs\/(\d+)-").unwrap())
                        .captures(&issue.body)
                        .and_then(|c| c.get(1))
                })
                .or_else(|| {
                    RE_RFC_TITLE
                        .get_or_init(|| {
                            Regex::new(r"Tracking [iI]ssue for? RFC (#|PR )?(\d+)").unwrap()
                        })
                        .captures(&issue.title)
                        .and_then(|c| c.get(2))
                }) {
                Some(rfc) => rfc.as_str().parse().expect("checked"),
                None => {
                    debug!(number = issue.number, "No RFC number found");
                    return None;
                }
            },
        };

        let label_events = label_events_for::<Label>(issue.timeline_items);

        let approved_at = label_events
            .iter()
            .find_map(|evt| match evt {
                LabelEvent::Applied {
                    label: Label::RfcApproved,
                    at,
                } => Some(*at),
                _ => None,
            })
            .unwrap_or(issue.created_at);

        let implemented_at = label_events.iter().find_map(|evt| match evt {
            LabelEvent::Applied {
                label: Label::RfcImplemented,
                at,
            } => Some(*at),
            _ => None,
        });

        Some(TrackingIssue {
            number: issue.number,
            title: issue.title,
            rfc,
            created_at: issue.created_at,
            approved_at,
            implemented_at,
            closed_at: issue.closed_at,
        })
    }
}

enum Label {
    RfcApproved,
    RfcImplemented,
}

impl FromStr for Label {
    type Err = ();

    fn from_str(label: &str) -> Result<Self, Self::Err> {
        match label {
            "B-RFC-approved" => Ok(Label::RfcApproved),
            "B-RFC-implemented" => Ok(Label::RfcImplemented),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Serialize)]
pub(super) struct Aggregate {
    date: DateTime<Utc>,
    created: usize,
    approved: usize,
    implemented: usize,
    closed: usize,
}

#[derive(Clone, Serialize)]
pub(super) struct Data {
    pub(super) agg: Vec<Aggregate>,
    pub(super) open: Vec<TrackingIssue>,
    pub(super) closed: Vec<TrackingIssue>,
}

impl Data {
    pub(super) fn new(tracking_issues: Vec<TrackingIssue>) -> Self {
        #[derive(PartialEq, Eq, PartialOrd, Ord)]
        enum Event {
            Created,
            Approved,
            Implemented,
            Closed,
        }

        let mut events = tracking_issues
            .iter()
            .flat_map(|issue| {
                [
                    (issue.created_at, Event::Created),
                    (issue.approved_at, Event::Approved),
                ]
                .into_iter()
                .chain(
                    issue
                        .implemented_at
                        .or(issue.closed_at)
                        .map(|d| (d, Event::Implemented)),
                )
                .chain(issue.closed_at.map(|d| (d, Event::Closed)))
            })
            .collect::<Vec<_>>();

        events.sort();

        let mut created = 0;
        let mut approved = 0;
        let mut implemented = 0;
        let mut closed = 0;
        let mut data = BTreeMap::new();

        for (date, event) in events {
            match event {
                Event::Created => created += 1,
                Event::Approved => {
                    created -= 1;
                    approved += 1;
                }
                Event::Implemented => {
                    approved -= 1;
                    implemented += 1;
                }
                Event::Closed => {
                    implemented -= 1;
                    closed += 1;
                }
            }
            *(data.entry(date).or_default()) = (created, approved, implemented, closed);
        }

        let open = tracking_issues
            .iter()
            .filter(|issue| issue.closed_at.is_none())
            .cloned()
            .collect();

        let closed = tracking_issues
            .into_iter()
            .filter(|issue| issue.closed_at.is_some())
            .collect();

        Self {
            agg: data
                .into_iter()
                .map(
                    |(date, (created, approved, implemented, closed))| Aggregate {
                        date,
                        created,
                        approved,
                        implemented,
                        closed,
                    },
                )
                .collect(),
            open,
            closed,
        }
    }
}
