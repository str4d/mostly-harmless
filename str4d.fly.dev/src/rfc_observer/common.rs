use std::str::FromStr;

use graphql_client::GraphQLQuery;

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
