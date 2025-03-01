use std::{cell::OnceCell, collections::BTreeMap, str::FromStr};

use chrono::{DateTime, NaiveDate, Utc};
use phf::phf_map;
use regex::Regex;
use serde::Serialize;
use tracing::debug;

use crate::rfc_observer::common::{
    issues_with_labels_and_body_query::IssuesWithLabelsAndBodyQueryRepositoryIssuesEdgesNode,
    label_events_for_bodied, LabelEvent,
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
    pub(super) closed_at: Option<DateTime<Utc>>,
    #[serde(skip)]
    label_events: Vec<LabelEvent<Label>>,
}

impl TrackingIssue {
    pub(super) fn new(
        issue: IssuesWithLabelsAndBodyQueryRepositoryIssuesEdgesNode,
    ) -> Option<Self> {
        if ISSUES_TO_IGNORE.contains(&issue.common.number) {
            return None;
        }

        // Attempt to identify the RFC.
        let rfc = match RFC_FOR_ISSUE.get(&issue.common.number) {
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
                        .captures(&issue.common.title)
                        .and_then(|c| c.get(2))
                }) {
                Some(rfc) => rfc.as_str().parse().expect("checked"),
                None => {
                    debug!(number = issue.common.number, "No RFC number found");
                    return None;
                }
            },
        };

        let label_events = label_events_for_bodied::<Label>(issue.common.timeline_items);

        Some(TrackingIssue {
            number: issue.common.number,
            title: issue.common.title,
            rfc,
            created_at: issue.common.created_at,
            closed_at: issue.common.closed_at,
            label_events,
        })
    }
}

#[derive(Clone, Copy, Debug)]
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

#[derive(Clone, Debug, Serialize)]
pub(super) struct Aggregate {
    date: NaiveDate,
    created: u64,
    approved: u64,
    implemented: u64,
    closed: u64,
}

#[derive(Clone, Serialize)]
pub(super) struct Data {
    pub(super) agg: Vec<Aggregate>,
    pub(super) open: Vec<TrackingIssue>,
    pub(super) closed: Vec<TrackingIssue>,
}

impl Data {
    pub(super) fn new(tracking_issues: Vec<TrackingIssue>) -> Self {
        enum State {
            Created,
            Approved,
            Implemented,
        }

        // First, build up a map of deltas for each day we have a label event on.
        #[derive(Debug, Default)]
        struct Deltas {
            created: i64,
            approved: i64,
            implemented: i64,
            closed: i64,
        }
        let mut deltas = BTreeMap::<_, Deltas>::new();
        fn day<'d>(
            deltas: &'d mut BTreeMap<NaiveDate, Deltas>,
            at: &DateTime<Utc>,
        ) -> &'d mut Deltas {
            deltas.entry(at.date_naive()).or_default()
        }

        for issue in &tracking_issues {
            let mut state = State::Created;
            day(&mut deltas, &issue.created_at).created += 1;

            for event in &issue.label_events {
                match event {
                    LabelEvent::Applied { at, label } => match (issue.closed_at, &mut state, label)
                    {
                        // Ignore label events after the tracking issue is closed.
                        (Some(closed), _, _) if closed <= *at => (),

                        // Ignore label events that don't change state.
                        (_, State::Approved, Label::RfcApproved)
                        | (_, State::Implemented, Label::RfcImplemented) => (),

                        // State transitions due to new labels.
                        (_, State::Created, Label::RfcApproved) => {
                            let d = day(&mut deltas, at);
                            d.created -= 1;
                            d.approved += 1;
                            state = State::Approved;
                        }
                        (_, State::Implemented, Label::RfcApproved) => {
                            let d = day(&mut deltas, at);
                            d.implemented -= 1;
                            d.approved += 1;
                            state = State::Approved;
                        }
                        (_, State::Created, Label::RfcImplemented) => {
                            let d = day(&mut deltas, at);
                            d.created -= 1;
                            d.implemented += 1;
                            state = State::Implemented;
                        }
                        (_, State::Approved, Label::RfcImplemented) => {
                            let d = day(&mut deltas, at);
                            d.approved -= 1;
                            d.implemented += 1;
                            state = State::Implemented;
                        }
                    },

                    LabelEvent::Removed { at, label } => match (issue.closed_at, &mut state, label)
                    {
                        // Ignore label events after the tracking issue is closed.
                        (Some(closed), _, _) if closed <= *at => (),

                        // Ignore label events that don't change state.
                        (_, State::Created | State::Implemented, Label::RfcApproved)
                        | (_, State::Created | State::Approved, Label::RfcImplemented) => (),

                        // State transitions due to removed labels.
                        (_, State::Approved, Label::RfcApproved) => {
                            let d = day(&mut deltas, at);
                            d.approved -= 1;
                            d.created += 1;
                            state = State::Created;
                        }
                        (_, State::Implemented, Label::RfcImplemented) => {
                            let d = day(&mut deltas, at);
                            d.implemented -= 1;
                            d.created += 1;
                            state = State::Created;
                        }
                    },
                }
            }

            if let Some(at) = issue.closed_at {
                let d = day(&mut deltas, &at);
                match state {
                    State::Created => d.created -= 1,
                    State::Approved => d.approved -= 1,
                    State::Implemented => d.implemented -= 1,
                }
                d.closed += 1;
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
                            created: deltas.created.try_into().expect("first day"),
                            approved: deltas.approved.try_into().expect("first day"),
                            implemented: deltas.implemented.try_into().expect("first day"),
                            closed: deltas.closed.try_into().expect("first day"),
                        })
                    }
                    Some(d) => {
                        d.date = date;
                        d.created = d.created.checked_add_signed(deltas.created).expect("fine");
                        d.approved = d
                            .approved
                            .checked_add_signed(deltas.approved)
                            .expect("fine");
                        d.implemented = d
                            .implemented
                            .checked_add_signed(deltas.implemented)
                            .expect("fine");
                        d.closed = d.closed.checked_add_signed(deltas.closed).expect("fine");
                    }
                }

                state.clone()
            })
            .collect();

        let open = tracking_issues
            .iter()
            .filter(|issue| issue.closed_at.is_none())
            .cloned()
            .collect();

        let mut closed = tracking_issues
            .into_iter()
            .filter(|issue| issue.closed_at.is_some())
            .collect::<Vec<_>>();

        // Sort closed issues by length of time they were open.
        closed.sort_by_cached_key(|issue| issue.closed_at.unwrap() - issue.created_at);
        closed.reverse();

        Self { agg, open, closed }
    }
}
