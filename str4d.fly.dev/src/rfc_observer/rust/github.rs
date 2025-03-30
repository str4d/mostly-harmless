use std::fmt;

use crate::{
    rfc_observer::common::{issues_with_labels_and_body_query, IssuesWithLabelsAndBodyQuery},
    util::github,
};

use super::data::TrackingIssue;

pub(super) async fn get_tracking_issues() -> Result<Vec<TrackingIssue>, Error> {
    let client = github::Client::new("rust.rfc.observer")?;

    let data = client
        .post_paginated_graphql::<IssuesWithLabelsAndBodyQuery>(
            issues_with_labels_and_body_query::Variables {
                owner: "rust-lang".into(),
                name: "rust".into(),
                labels: vec!["B-RFC-approved".into(), "B-RFC-implemented".into()],
                after: None,
            },
        )
        .await?
        .into_data()
        .map_err(Error::GraphQl)?;

    let repo = data.repository.expect("repo exists");

    let mut tracking_issues = repo
        .issues
        .edges
        .into_iter()
        .flat_map(|issues| issues.into_iter())
        .map(|e| e.and_then(|edge| edge.node))
        .flatten()
        .flat_map(|issue| TrackingIssue::new(issue))
        .collect::<Vec<_>>();

    tracking_issues.sort_by_key(|issue| (issue.rfc, issue.created_at, issue.closed_at));

    Ok(tracking_issues)
}

#[derive(Debug)]
pub(super) enum Error {
    GitHub(github::Error),
    GraphQl(Vec<graphql_client::Error>),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::GitHub(e) => write!(f, "GitHub error: {e}"),
            Error::GraphQl(errors) => {
                writeln!(f, "GraphQL errors: [")?;
                for e in errors {
                    writeln!(f, "{e},")?;
                }
                write!(f, "]")
            }
        }
    }
}

impl From<github::Error> for Error {
    fn from(err: github::Error) -> Self {
        Error::GitHub(err)
    }
}
