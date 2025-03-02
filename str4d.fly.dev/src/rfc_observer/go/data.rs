use std::{collections::BTreeMap, str::FromStr};

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;

use crate::rfc_observer::common::{
    completion_months_histogram,
    issues_with_labels_query::IssuesWithLabelsQueryRepositoryIssuesEdgesNode, label_events_for,
    Bucket, HistogramStats, LabelEvent,
};

#[derive(Clone, Debug, Serialize)]
pub(super) struct Proposal {
    pub(super) number: i64,
    title: String,
    created_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
    #[serde(skip)]
    label_events: Vec<LabelEvent<Label>>,
}

impl Proposal {
    pub(super) fn new(issue: IssuesWithLabelsQueryRepositoryIssuesEdgesNode) -> Option<Self> {
        let label_events = label_events_for::<Label>(issue.timeline_items);

        Some(Proposal {
            number: issue.number,
            title: issue.title,
            created_at: issue.created_at,
            closed_at: issue.closed_at,
            label_events,
        })
    }
}

#[derive(Clone, Copy, Debug)]
enum Label {
    Proposal,
    ProposalHold,
    ProposalAccepted,
}

impl FromStr for Label {
    type Err = ();

    fn from_str(label: &str) -> Result<Self, Self::Err> {
        match label {
            "Proposal" => Ok(Label::Proposal),
            "Proposal-Hold" => Ok(Label::ProposalHold),
            "Proposal-Accepted" => Ok(Label::ProposalAccepted),
            _ => Err(()),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct Aggregate {
    date: NaiveDate,
    created: u64,
    on_hold: u64,
    accepted: u64,
    implemented: u64,
    closed: u64,
}

#[derive(Clone, Serialize)]
pub(super) struct Data {
    pub(super) agg: Vec<Aggregate>,
    pub(super) completed_hist: Vec<Bucket>,
    pub(super) completed_stats: HistogramStats,
    pub(super) open: Vec<Proposal>,
    pub(super) closed: Vec<Proposal>,
}

impl Data {
    pub(super) fn new(proposals: Vec<Proposal>) -> Self {
        enum State {
            Created,
            OnHold,
            Accepted,
        }

        // First, build up a map of deltas for each day we have a label event on.
        #[derive(Debug, Default)]
        struct Deltas {
            created: i64,
            on_hold: i64,
            accepted: i64,
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

        for issue in &proposals {
            let mut state = State::Created;
            day(&mut deltas, &issue.created_at).created += 1;

            for event in &issue.label_events {
                match event {
                    LabelEvent::Applied { at, label } => match (issue.closed_at, &mut state, label)
                    {
                        // Ignore label events after the tracking issue is closed.
                        (Some(closed), _, _) if closed <= *at => (),

                        // Ignore label events that don't change state.
                        (_, _, Label::Proposal)
                        | (_, State::OnHold, Label::ProposalHold)
                        | (_, State::Accepted, Label::ProposalAccepted) => (),

                        // State transitions due to new labels.
                        (_, State::Created, Label::ProposalHold) => {
                            let d = day(&mut deltas, at);
                            d.created -= 1;
                            d.on_hold += 1;
                            state = State::OnHold;
                        }
                        (_, State::Accepted, Label::ProposalHold) => {
                            let d = day(&mut deltas, at);
                            d.accepted -= 1;
                            d.on_hold += 1;
                            state = State::OnHold;
                        }
                        (_, State::Created, Label::ProposalAccepted) => {
                            let d = day(&mut deltas, at);
                            d.created -= 1;
                            d.accepted += 1;
                            state = State::Accepted;
                        }
                        (_, State::OnHold, Label::ProposalAccepted) => {
                            let d = day(&mut deltas, at);
                            d.on_hold -= 1;
                            d.accepted += 1;
                            state = State::Accepted;
                        }
                    },

                    LabelEvent::Removed { at, label } => match (issue.closed_at, &mut state, label)
                    {
                        // Ignore label events after the tracking issue is closed.
                        (Some(closed), _, _) if closed <= *at => (),

                        // Ignore label events that don't change state.
                        (_, _, Label::Proposal)
                        | (_, State::Created | State::Accepted, Label::ProposalHold)
                        | (_, State::Created | State::OnHold, Label::ProposalAccepted) => (),

                        // State transitions due to removed labels.
                        (_, State::OnHold, Label::ProposalHold) => {
                            let d = day(&mut deltas, at);
                            d.on_hold -= 1;
                            d.created += 1;
                            state = State::Created;
                        }
                        (_, State::Accepted, Label::ProposalAccepted) => {
                            let d = day(&mut deltas, at);
                            d.accepted -= 1;
                            d.created += 1;
                            state = State::Created;
                        }
                    },
                }
            }

            if let Some(at) = issue.closed_at {
                let d = day(&mut deltas, &at);
                match state {
                    State::Created => {
                        d.created -= 1;
                        d.closed += 1;
                    }
                    State::OnHold => {
                        d.on_hold -= 1;
                        d.closed += 1;
                    }
                    State::Accepted => {
                        d.accepted -= 1;
                        d.implemented += 1;
                    }
                }
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
                            on_hold: deltas.on_hold.try_into().expect("first day"),
                            accepted: deltas.accepted.try_into().expect("first day"),
                            implemented: deltas.implemented.try_into().expect("first day"),
                            closed: deltas.closed.try_into().expect("first day"),
                        })
                    }
                    Some(d) => {
                        d.date = date;
                        d.created = d.created.checked_add_signed(deltas.created).expect("fine");
                        d.on_hold = d.on_hold.checked_add_signed(deltas.on_hold).expect("fine");
                        d.accepted = d
                            .accepted
                            .checked_add_signed(deltas.accepted)
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

        let open = proposals
            .iter()
            .filter(|issue| issue.closed_at.is_none())
            .cloned()
            .collect();

        let mut closed = proposals
            .into_iter()
            .filter(|issue| issue.closed_at.is_some())
            .collect::<Vec<_>>();

        // Sort closed issues by length of time they were open.
        closed.sort_by_cached_key(|proposal| proposal.closed_at.unwrap() - proposal.created_at);
        closed.reverse();

        // Prepare a histogram of "number of proposals completed within X months".
        let (completed_hist, completed_stats) = completion_months_histogram(&closed, |proposal| {
            (&proposal.created_at, &proposal.closed_at.as_ref().unwrap())
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
