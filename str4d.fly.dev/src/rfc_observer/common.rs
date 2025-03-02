use std::{collections::BTreeMap, str::FromStr};

use chrono::{Datelike, Months};
use graphql_client::GraphQLQuery;
use serde::Serialize;

use crate::util::github;

type DateTime = chrono::DateTime<chrono::Utc>;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "res/graphql/github-schema.graphql",
    query_path = "res/graphql/rfc-observer-query.graphql",
    response_derives = "Debug"
)]
pub struct IssuesWithLabelsQuery;

impl github::PaginatedQuery for IssuesWithLabelsQuery {
    fn page_info(data: &Self::ResponseData) -> github::PageInfo {
        let page_info = &data
            .repository
            .as_ref()
            .expect("repo exists")
            .issues
            .page_info;

        github::PageInfo {
            end_cursor: page_info.end_cursor.clone(),
            has_next_page: page_info.has_next_page,
        }
    }

    fn with_after(variables: &Self::Variables, after: Option<String>) -> Self::Variables {
        issues_with_labels_query::Variables {
            owner: variables.owner.clone(),
            name: variables.name.clone(),
            labels: variables.labels.clone(),
            after,
        }
    }

    fn merge_page(acc: &mut Self::ResponseData, page: Self::ResponseData) {
        let issues = &mut acc.repository.as_mut().expect("repo exists").issues;

        match (
            issues.edges.as_mut(),
            page.repository.expect("repo exists").issues.edges,
        ) {
            (_, None) => (),
            (None, Some(edges)) => issues.edges = Some(edges),
            (Some(acc), Some(mut page)) => acc.append(&mut page),
        }
    }
}

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "res/graphql/github-schema.graphql",
    query_path = "res/graphql/rfc-observer-query.graphql",
    response_derives = "Debug"
)]
pub struct IssuesWithLabelsAndBodyQuery;

impl github::PaginatedQuery for IssuesWithLabelsAndBodyQuery {
    fn page_info(data: &Self::ResponseData) -> github::PageInfo {
        let page_info = &data
            .repository
            .as_ref()
            .expect("repo exists")
            .issues
            .page_info;

        github::PageInfo {
            end_cursor: page_info.end_cursor.clone(),
            has_next_page: page_info.has_next_page,
        }
    }

    fn with_after(variables: &Self::Variables, after: Option<String>) -> Self::Variables {
        issues_with_labels_and_body_query::Variables {
            owner: variables.owner.clone(),
            name: variables.name.clone(),
            labels: variables.labels.clone(),
            after,
        }
    }

    fn merge_page(acc: &mut Self::ResponseData, page: Self::ResponseData) {
        let issues = &mut acc.repository.as_mut().expect("repo exists").issues;

        match (
            issues.edges.as_mut(),
            page.repository.expect("repo exists").issues.edges,
        ) {
            (_, None) => (),
            (None, Some(edges)) => issues.edges = Some(edges),
            (Some(acc), Some(mut page)) => acc.append(&mut page),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) enum LabelEvent<L> {
    Applied { at: DateTime, label: L },
    Removed { at: DateTime, label: L },
}

pub(super) fn label_events_for<L: FromStr>(
    timeline_items: issues_with_labels_query::CommonTimelineItems,
) -> Vec<LabelEvent<L>> {
    use self::issues_with_labels_query::CommonTimelineItemsEdgesNode::{
        LabeledEvent, UnlabeledEvent,
    };

    let mut ret = timeline_items
        .edges
        .into_iter()
        .flat_map(|events| events.into_iter().flatten().filter_map(|event| event.node))
        .filter_map(|event| match event {
            LabeledEvent(e) => e.label.name.parse().ok().map(|label| LabelEvent::Applied {
                at: e.created_at,
                label,
            }),
            UnlabeledEvent(e) => e.label.name.parse().ok().map(|label| LabelEvent::Removed {
                at: e.created_at,
                label,
            }),
            _ => None,
        })
        .collect::<Vec<_>>();

    ret.sort_by_key(|e| match e {
        LabelEvent::Applied { at, .. } => *at,
        LabelEvent::Removed { at, .. } => *at,
    });

    ret
}

pub(super) fn label_events_for_bodied<L: FromStr>(
    timeline_items: issues_with_labels_and_body_query::CommonTimelineItems,
) -> Vec<LabelEvent<L>> {
    use self::issues_with_labels_and_body_query::CommonTimelineItemsEdgesNode::{
        LabeledEvent, UnlabeledEvent,
    };

    let mut ret = timeline_items
        .edges
        .into_iter()
        .flat_map(|events| events.into_iter().flatten().filter_map(|event| event.node))
        .filter_map(|event| match event {
            LabeledEvent(e) => e.label.name.parse().ok().map(|label| LabelEvent::Applied {
                at: e.created_at,
                label,
            }),
            UnlabeledEvent(e) => e.label.name.parse().ok().map(|label| LabelEvent::Removed {
                at: e.created_at,
                label,
            }),
            _ => None,
        })
        .collect::<Vec<_>>();

    ret.sort_by_key(|e| match e {
        LabelEvent::Applied { at, .. } => *at,
        LabelEvent::Removed { at, .. } => *at,
    });

    ret
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct Bucket {
    label: u32,
    count: u64,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct HistogramStats {
    median: u32,
}

pub(super) fn completion_months_histogram<D>(
    rfcs: &[D],
    open_range: impl Fn(&D) -> (&DateTime, &DateTime),
) -> (Vec<Bucket>, HistogramStats) {
    let mut hist = BTreeMap::new();
    for rfc in rfcs {
        let (start, end) = open_range(rfc);

        let months_open = {
            // If the end's month is earlier than the start, this will over-count by a
            // year, but that is fixed by the subsequent months calculation.
            let years_open = u32::try_from(end.year() - start.year()).unwrap_or(0);
            let mut months_open = years_open * 12 + end.month() - start.month();

            // Handle the possible over-counting if the end's day-within-month is earlier
            // than the start's.
            let earlier_time = (end.day(), end.time()) < (start.day(), start.time());
            months_open -= match earlier_time {
                true => 1,
                false => 0,
            };

            // If the number of days open beyond a month boundary is more than two weeks,
            // round up.
            let remaining = (*end - Months::new(months_open)) - start;
            months_open
                + match remaining.num_seconds() > (86400 * 14) {
                    true => 1,
                    false => 0,
                }
        };

        *hist.entry(months_open).or_default() += 1;
    }

    // Fill in the gaps.
    if let Some((&top_bucket, _)) = hist.last_key_value() {
        for i in 0..top_bucket {
            hist.entry(i).or_default();
        }
    }

    let median_count = rfcs.len() as u64 / 2;
    let (median, total_count) = hist
        .iter()
        .fold((0, 0), |(median, acc_count), (label, count)| {
            let new_count = acc_count + count;
            if new_count > median_count {
                (median, new_count)
            } else {
                (*label, new_count)
            }
        });
    assert_eq!(total_count, rfcs.len() as u64);

    (
        hist.into_iter()
            .map(|(label, count)| Bucket { label, count })
            .collect(),
        HistogramStats { median },
    )
}
