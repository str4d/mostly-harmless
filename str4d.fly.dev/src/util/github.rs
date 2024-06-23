use std::{env, fmt, iter};

use graphql_client::{GraphQLQuery, Response};
use reqwest::header::{HeaderValue, AUTHORIZATION};
use tracing::debug;

const API_URL: &str = "https://api.github.com/graphql";

pub struct Client {
    inner: reqwest::Client,
}

impl Client {
    pub fn new(user_agent: &str) -> Result<Self, Error> {
        let api_key = env::var("GITHUB_API_KEY").map_err(|_| Error::GitHubApiKeyMissing)?;
        let mut bearer_auth = HeaderValue::from_str(&format!("Bearer {api_key}"))
            .map_err(|_| Error::GitHubApiKeyInvalid)?;
        bearer_auth.set_sensitive(true);

        let inner = reqwest::Client::builder()
            .user_agent(user_agent)
            .default_headers(iter::once((AUTHORIZATION, bearer_auth)).collect())
            .build()?;

        Ok(Self { inner })
    }

    pub async fn post_graphql<Q: GraphQLQuery>(
        &self,
        variables: Q::Variables,
    ) -> Result<GraphQlResponse<Q>, Error> {
        let request_body = Q::build_query(variables);

        let res = self
            .inner
            .post(API_URL)
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;

        Ok(GraphQlResponse {
            inner: res.json().await?,
        })
    }
}

pub struct GraphQlResponse<Q: GraphQLQuery> {
    inner: Response<Q::ResponseData>,
}

impl<Q: GraphQLQuery> GraphQlResponse<Q> {
    pub fn into_data(self) -> Result<Q::ResponseData, Vec<graphql_client::Error>> {
        Ok(self.inner.data.ok_or_else(|| {
            if let Some(errors) = self.inner.errors {
                errors
            } else {
                vec![]
            }
        })?)
    }
}

pub struct PageInfo {
    pub end_cursor: Option<String>,
    pub has_next_page: bool,
}

pub trait PaginatedQuery: GraphQLQuery {
    fn page_info(data: &Self::ResponseData) -> PageInfo;
    fn with_after(variables: &Self::Variables, after: Option<String>) -> Self::Variables;
    fn merge_page(acc: &mut Self::ResponseData, page: Self::ResponseData);
}

impl Client {
    pub async fn post_paginated_graphql<Q: PaginatedQuery>(
        &self,
        variables: Q::Variables,
    ) -> Result<GraphQlResponse<Q>, Error> {
        // Fetch the first page.
        debug!("Fetching first page");
        let mut response = self.post_graphql(Q::with_after(&variables, None)).await?;

        match &mut response.inner.data {
            None => return Ok(response),
            Some(data) => {
                let mut page_info = Q::page_info(&data);

                while page_info.has_next_page {
                    // Fetch the next page.
                    debug!("Fetching next page");
                    let next_response = self
                        .post_graphql::<Q>(Q::with_after(&variables, page_info.end_cursor))
                        .await?;

                    // Merge errors into response.
                    match (response.inner.errors.as_mut(), next_response.inner.errors) {
                        (_, None) => (),
                        (None, Some(errors)) => response.inner.errors = Some(errors),
                        (Some(cur), Some(mut add)) => cur.append(&mut add),
                    }

                    // Merge data into response.
                    match next_response.inner.data {
                        None => break,
                        Some(page_data) => {
                            page_info = Q::page_info(&page_data);
                            Q::merge_page(data, page_data);
                        }
                    }
                }
            }
        }

        Ok(response)
    }
}

#[derive(Debug)]
pub enum Error {
    GitHubApiKeyInvalid,
    GitHubApiKeyMissing,
    Request(reqwest::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::GitHubApiKeyInvalid => {
                write!(f, "GITHUB_API_KEY environment variable is invalid")
            }
            Error::GitHubApiKeyMissing => {
                write!(f, "GITHUB_API_KEY environment variable is missing")
            }
            Error::Request(e) => write!(f, "Error while processing request: {}", e),
        }
    }
}

impl std::error::Error for Error {}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::Request(err)
    }
}
