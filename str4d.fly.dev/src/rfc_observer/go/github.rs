use std::fmt;

use crate::{
    rfc_observer::common::{IssuesWithLabelsQuery, issues_with_labels_query},
    util::github,
};

use super::data::Proposal;

pub(super) async fn get_proposals() -> Result<Vec<Proposal>, Error> {
    let client = github::Client::new("go.rfc.observer")?;

    let data = client
        .post_paginated_graphql::<IssuesWithLabelsQuery>(issues_with_labels_query::Variables {
            owner: "golang".into(),
            name: "go".into(),
            labels: vec![
                "Proposal".into(),
                "Proposal-Hold".into(),
                "Proposal-Accepted".into(),
            ],
            after: None,
        })
        .await?
        .into_data()
        .map_err(Error::GraphQl)?;

    let repo = data.repository.expect("repo exists");

    let mut proposals = repo
        .issues
        .edges
        .into_iter()
        .flat_map(|issues| issues.into_iter())
        .map(|e| e.and_then(|edge| edge.node))
        .flatten()
        .flat_map(|issue| Proposal::new(issue))
        .collect::<Vec<_>>();

    proposals.sort_by_key(|issue| (issue.number));

    Ok(proposals)
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
