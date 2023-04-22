use hyper::StatusCode;
use serde::Deserialize;

fn query_url(method_id: &str) -> String {
    format!("https://bsky.social/xrpc/{}", method_id)
}

pub(super) struct Client {
    client: reqwest::Client,
}

impl Client {
    pub(super) fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    async fn query(&self, method_id: &str) -> Result<reqwest::Response, Error> {
        let resp = self.client.get(query_url(method_id)).send().await?;
        let status = resp.status();

        if status.is_success() {
            Ok(resp)
        } else {
            Err(if status == StatusCode::UNAUTHORIZED {
                Error::AuthRequired
            } else if status == StatusCode::FORBIDDEN {
                Error::Forbidden
            } else if status == StatusCode::NOT_FOUND
                || status.is_informational()
                || status.is_redirection()
            {
                Error::XrpcNotSupported
            } else if status == StatusCode::PAYLOAD_TOO_LARGE {
                Error::PayloadTooLarge
            } else if status == StatusCode::TOO_MANY_REQUESTS {
                Error::RateLimitExceeded
            } else if status == StatusCode::BAD_REQUEST || status.is_client_error() {
                Error::InvalidRequest
            } else if status == StatusCode::NOT_IMPLEMENTED {
                Error::MethodNotImplemented
            } else if status == StatusCode::BAD_GATEWAY {
                Error::UpstreamRequestFailed
            } else if status == StatusCode::SERVICE_UNAVAILABLE {
                Error::NotEnoughResources
            } else if status == StatusCode::GATEWAY_TIMEOUT {
                Error::UpstreamRequestTimeout
            } else if status == StatusCode::INTERNAL_SERVER_ERROR || status.is_server_error() {
                Error::InternalServerError
            } else {
                unreachable!()
            })
        }
    }

    pub(super) async fn get_feed(&self) -> Result<Vec<Post>, Error> {
        let resp = self
            .query("com.atproto.repo.listRecords?repo=siso.dev&collection=app.bsky.feed.post")
            .await?
            .json::<Records>()
            .await?;

        Ok(resp.records.into_iter().map(|r| r.value).collect())
    }
}

#[derive(Deserialize)]
struct Records {
    records: Vec<Record>,
}

#[derive(Deserialize)]
struct Record {
    value: Post,
}

#[derive(Clone, Deserialize)]
pub(super) struct Post {
    pub(super) text: String,
    embed: Option<PostEmbed>,
    #[serde(rename = "createdAt")]
    pub(super) created_at: chrono::DateTime<chrono::Utc>,
}

impl Post {
    pub(super) fn images(&self) -> impl Iterator<Item = (String, &String)> + '_ {
        self.embed.iter().flat_map(|embed| {
            embed.images.iter().map(|i| {
                let link = &i.image.reference.link;

                (
                    query_url(&format!(
                        "com.atproto.sync.getBlob?did=did:plc:mzwculbn44rdeouyzjp4y6gx&cid={}",
                        link
                    )),
                    &i.alt,
                )
            })
        })
    }

    pub(super) fn ago(&self) -> String {
        timeago::Formatter::new().convert_chrono(self.created_at, chrono::Utc::now())
    }
}

#[derive(Clone, Deserialize)]
struct PostEmbed {
    images: Vec<PostImage>,
}

#[derive(Clone, Deserialize)]
struct PostImage {
    alt: String,
    image: PostImageData,
}

#[derive(Clone, Deserialize)]
struct PostImageData {
    #[serde(rename = "ref")]
    reference: PostImageRef,
}

#[derive(Clone, Deserialize)]
struct PostImageRef {
    #[serde(rename = "$link")]
    link: String,
}

#[derive(Debug)]
pub(super) enum Error {
    Reqwest(reqwest::Error),
    /// The request is invalid and was not processed.
    InvalidRequest,
    /// The request cannot be processed without authentication.
    AuthRequired,
    /// The user lacks the needed permissions to access the method.
    Forbidden,
    XrpcNotSupported,
    /// The payload of the request is larger than the server is willing to process.
    /// Payload size-limits are decided by each server.
    PayloadTooLarge,
    /// The client has sent too many requests. Rate-limits are decided by each server.
    RateLimitExceeded,
    /// The server reached an unexpected condition during processing.
    InternalServerError,
    /// The server does not implement the requested method.
    MethodNotImplemented,
    /// The execution of the procedure depends on a call to another server which has failed.
    UpstreamRequestFailed,
    /// The server is under heavy load and can't complete the request.
    NotEnoughResources,
    /// The execution of the procedure depends on a call to another server which timed out.
    UpstreamRequestTimeout,
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Reqwest(e)
    }
}
