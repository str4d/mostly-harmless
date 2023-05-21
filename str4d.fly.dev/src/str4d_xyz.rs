use std::collections::HashMap;

use askama::Template;
use axum::{extract::Query, response::Redirect, routing::get, Router};

use crate::util::get_temp_redir;

pub(crate) fn build() -> Router {
    Router::new()
        .route("/", get(index))
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
