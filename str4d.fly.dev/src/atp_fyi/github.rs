use std::env;

use graphql_client::GraphQLQuery;

use crate::util::github;

use self::social_app_query::SocialAppQueryRepositoryIssuesEdgesNodeLabelsEdges;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "res/graphql/github-schema.graphql",
    query_path = "res/graphql/social-app-query.graphql"
)]
pub struct SocialAppQuery;

impl github::PaginatedQuery for SocialAppQuery {
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

    fn with_after(_: &Self::Variables, after: Option<String>) -> Self::Variables {
        social_app_query::Variables { after }
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

pub(super) async fn get_roadmap() -> Result<Roadmap, Error> {
    let client = github::Client::new("atp.fyi")?;

    let data = client
        .post_paginated_graphql::<SocialAppQuery>(social_app_query::Variables { after: None })
        .await?
        .into_data()
        .map_err(Error::GraphQL)?;

    let repo = data.repository.expect("repo exists");

    let mut roadmap = Roadmap::default();

    for (label, author, issue) in repo
        .issues
        .edges
        .into_iter()
        .flat_map(|issues| issues.into_iter())
        .map(|e| e.and_then(|edge| edge.node))
        .flatten()
        .filter_map(|issue| {
            issue
                .labels
                .and_then(|labels| labels.edges)
                .and_then(|labels| labels.into_iter().find_map(Label::parse))
                .map(|label| {
                    (
                        label,
                        Author::parse(issue.author),
                        Issue::new(issue.number, issue.title),
                    )
                })
        })
    {
        match label {
            Label::Discussing => roadmap.discussing.push(author, issue),
            Label::OnTheRoadmap => roadmap.on_roadmap.push(author, issue),
            Label::Planned => roadmap.planned.push(author, issue),
            Label::PuttingThisOff => roadmap.putting_off.push(author, issue),
        }
    }

    Ok(roadmap)
}

enum Label {
    Discussing,
    OnTheRoadmap,
    Planned,
    PuttingThisOff,
}

impl Label {
    fn parse(val: Option<SocialAppQueryRepositoryIssuesEdgesNodeLabelsEdges>) -> Option<Self> {
        let label = val?.node?.name;
        match label.as_str() {
            "x:discussing" => Some(Label::Discussing),
            "x:on-the-roadmap" => Some(Label::OnTheRoadmap),
            "x:planned" => Some(Label::Planned),
            "x:putting-this-off" => Some(Label::PuttingThisOff),
            _ => None,
        }
    }
}

enum Author {
    Devs,
    Community,
}

impl Author {
    fn parse(val: Option<social_app_query::SocialAppQueryRepositoryIssuesEdgesNodeAuthor>) -> Self {
        val.map_or(Author::Community, |author| match author.login.as_str() {
            "ansh" | "bnewbold" | "devinivy" | "dholms" | "emilyliu7321" | "ericvolp12"
            | "estrattonbailey" | "haileyok" | "gaearon" | "Jacob2161" | "matthieusieben"
            | "mozzius" | "pfrazee" | "renahlee" | "whyrusleeping" => Author::Devs,
            _ => Author::Community,
        })
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct Roadmap {
    pub(super) discussing: Issues,
    pub(super) planned: Issues,
    pub(super) on_roadmap: Issues,
    pub(super) putting_off: Issues,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Issues {
    pub(super) devs: Vec<Issue>,
    pub(super) community: Vec<Issue>,
}

impl Issues {
    fn push(&mut self, author: Author, issue: Issue) {
        match author {
            Author::Devs => self.devs.push(issue),
            Author::Community => self.community.push(issue),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Issue {
    number: i64,
    title: String,
}

impl Issue {
    fn new(number: i64, title: String) -> Self {
        Issue { number, title }
    }

    pub(super) fn title(&self) -> &str {
        &self.title
    }

    pub(super) fn uri(&self) -> String {
        format!(
            "https://github.com/bluesky-social/social-app/issues/{}",
            self.number
        )
    }
}

#[derive(Debug)]
pub(super) enum Error {
    GitHub(github::Error),
    GraphQL(Vec<graphql_client::Error>),
    UnknownGraphQLError,
    Var(env::VarError),
}

impl From<github::Error> for Error {
    fn from(err: github::Error) -> Self {
        Error::GitHub(err)
    }
}

impl From<env::VarError> for Error {
    fn from(err: env::VarError) -> Self {
        Error::Var(err)
    }
}
