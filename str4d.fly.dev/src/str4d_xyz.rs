use std::collections::HashMap;

use askama::Template;
use axum::{extract::Query, response::Redirect, routing::get, Router};

use crate::util::get_temp_redir;

pub(crate) fn build() -> Router {
    Router::new()
        .route("/", get(index))
        .nest("/blog", blog())
        .nest("/rage", github_project_with_clone("str4d/rage"))
        .nest("/wage", github_project("str4d/wage"))
        .nest(
            "/age-plugin-yubikey",
            github_project("str4d/age-plugin-yubikey"),
        )
}

#[derive(Template)]
#[template(path = "str4d.xyz/index.html")]
struct Index {}

async fn index() -> Index {
    Index {}
}

fn blog() -> Router {
    Router::new()
        .route("/", get_temp_redir("https://words.str4d.xyz"))
        .route(
            "/posts/first-post/",
            get_temp_redir("https://words.str4d.xyz/first-post/"),
        )
        .route(
            "/posts/ignore-request-urls-in-jetty/",
            get_temp_redir("https://words.str4d.xyz/ignore-request-urls-in-jetty/"),
        )
        .route(
            "/posts/passing-custom-options-through-I2CP/",
            get_temp_redir("https://words.str4d.xyz/passing-custom-options-through-I2CP/"),
        )
        .route(
            "/posts/i2p-android-dev-builds/",
            get_temp_redir("https://words.str4d.xyz/i2p-android-dev-builds/"),
        )
        .route(
            "/posts/i2p-android-dev-the-second/",
            get_temp_redir("https://words.str4d.xyz/i2p-android-dev-the-second/"),
        )
        .route(
            "/posts/i2p-android-dev-the-third/",
            get_temp_redir("https://words.str4d.xyz/i2p-android-dev-the-third/"),
        )
        .route(
            "/posts/i2p-android-dev-the-fourth/",
            get_temp_redir("https://words.str4d.xyz/i2p-android-dev-the-fourth/"),
        )
        .route(
            "/posts/gpg-key-transition/",
            get_temp_redir("https://words.str4d.xyz/gpg-key-transition/"),
        )
}

fn github_project(project: &str) -> Router {
    Router::new()
        .route(
            "/",
            get_temp_redir(&format!("https://github.com/{}", project)),
        )
        .route(
            "/report",
            get_temp_redir(&format!("https://github.com/{}/issues/new/choose", project)),
        )
}

fn github_project_with_clone(project: &'static str) -> Router {
    github_project(project).route(
        "/info/refs",
        get(
            move |Query(params): Query<HashMap<String, String>>| async move {
                let mut uri = format!("https://github.com/{}.git/info/refs", project);
                for (i, (key, value)) in params.into_iter().enumerate() {
                    uri.push(if i == 0 { '?' } else { '&' });
                    uri.push_str(&key);
                    uri.push('=');
                    uri.push_str(&value);
                }
                Redirect::temporary(&uri)
            },
        ),
    )
}
