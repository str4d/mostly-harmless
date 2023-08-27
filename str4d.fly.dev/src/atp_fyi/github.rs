use std::env;

use graphql_client::{GraphQLQuery, Response};

use self::social_app_query::SocialAppQueryRepositoryIssuesEdgesNodeLabelsEdges;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "res/graphql/github-schema.json",
    query_path = "res/graphql/social-app-query.graphql"
)]
pub struct SocialAppQuery;

pub(super) async fn get_roadmap() -> Result<Roadmap, Error> {
    let variables = social_app_query::Variables;
    let request_body = SocialAppQuery::build_query(variables);

    let client = reqwest::ClientBuilder::new()
        .user_agent("atp.fyi")
        .build()?;
    let res = client
        .post("https://api.github.com/graphql")
        .bearer_auth(env::var("GITHUB_API_KEY")?)
        .json(&request_body)
        .send()
        .await?
        .error_for_status()?;
    let response_body: Response<social_app_query::ResponseData> = res.json().await?;

    let data = response_body.data.ok_or_else(|| {
        if let Some(errors) = response_body.errors {
            Error::GraphQL(errors)
        } else {
            Error::UnknownGraphQLError
        }
    })?;
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
            "ansh" | "bnewbold" | "devinivy" | "dholms" | "emilyliu7321" | "estrattonbailey"
            | "Jacob2161" | "pfrazee" | "renahlee" => Author::Devs,
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
    Reqwest(reqwest::Error),
    GraphQL(Vec<graphql_client::Error>),
    UnknownGraphQLError,
    Var(env::VarError),
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::Reqwest(err)
    }
}

impl From<env::VarError> for Error {
    fn from(err: env::VarError) -> Self {
        Error::Var(err)
    }
}
