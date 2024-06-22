use askama::Template;
use atrium_api::{
    app::{self, bsky::feed::post::RecordEmbedRefs::AppBskyEmbedImagesMain},
    client::AtpServiceClient,
    com::atproto::repo::list_records,
    types::{BlobRef, TypedBlobRef, Union},
};
use atrium_xrpc_client::reqwest::ReqwestClientBuilder;
use axum::{routing::get, Router};
use cached::proc_macro::cached;

pub(crate) fn build() -> Router {
    Router::new().route("/", get(index))
}

#[derive(Clone, Template)]
#[template(path = "siso.dev/index.html")]
struct Index {
    feed: Vec<(String, Post)>,
}

#[cached(time = 60)]
async fn index() -> Index {
    let feed = match get_feed().await {
        Ok(feed) => feed,
        Err(e) => {
            tracing::error!("Failed to get feed: {:?}", e);
            vec![]
        }
    };
    Index { feed }
}

async fn get_feed() -> anyhow::Result<Vec<(String, Post)>> {
    let client = AtpServiceClient::new(
        ReqwestClientBuilder::new("https://bsky.social")
            .client(reqwest::ClientBuilder::new().use_rustls_tls().build()?)
            .build(),
    );

    let feed = client
        .service
        .com
        .atproto
        .repo
        .list_records(
            list_records::ParametersData {
                collection: "app.bsky.feed.post".parse().expect("valid"),
                cursor: None,
                limit: Some(10.try_into().expect("valid")),
                repo: "siso.dev".parse().expect("valid"),
                reverse: None,
                rkey_end: None,
                rkey_start: None,
            }
            .into(),
        )
        .await?;

    Ok(feed
        .data
        .records
        .into_iter()
        .filter_map(|r| match r.data.value {
            atrium_api::records::Record::Known(
                atrium_api::records::KnownRecord::AppBskyFeedPost(post),
            ) => Some((r.data.cid.as_ref().to_string(), Post(*post))),
            _ => None,
        })
        .collect())
}

#[derive(Clone)]
pub(super) struct Post(app::bsky::feed::post::Record);

impl Post {
    pub(super) fn created_at(&self) -> &chrono::DateTime<chrono::FixedOffset> {
        self.0.created_at.as_ref()
    }

    pub(super) fn formatted_text(&self) -> String {
        format!(
            "<p>{}</p>",
            self.0.text.replace("\n\n", "</p><p>").replace("\n", "<br>")
        )
    }

    pub(super) fn images(&self) -> impl Iterator<Item = (String, &String)> + '_ {
        self.0
            .embed
            .iter()
            .filter_map(|embed| {
                if let Union::Refs(AppBskyEmbedImagesMain(embed)) = embed {
                    Some(embed.images.iter())
                } else {
                    None
                }
            })
            .flatten()
            .map(|i| {
                let link = match &i.image {
                    BlobRef::Typed(TypedBlobRef::Blob(blob_ref)) => &blob_ref.r#ref.0.to_string(),
                    BlobRef::Untyped(blob_ref) => &blob_ref.cid,
                };

                (
                    format!(
                        "https://cdn.bsky.app/img/feed_thumbnail/plain/{}/{}@jpeg",
                        "did:plc:mzwculbn44rdeouyzjp4y6gx", link,
                    ),
                    &i.alt,
                )
            })
    }

    pub(super) fn ago(&self) -> String {
        timeago::Formatter::new().convert_chrono(*self.0.created_at.as_ref(), chrono::Utc::now())
    }
}
