use axum::Router;

use crate::util::get_temp_redir;

pub(crate) fn www() -> Router {
    Router::new()
        .route("/", get_temp_redir("https://str4d.xyz"))
        .route("/about/", get_temp_redir("https://str4d.xyz"))
        .route("/contact/", get_temp_redir("https://str4d.xyz"))
        .route("/projects/", get_temp_redir("https://str4d.xyz"))
        .route(
            "/2011/04/20/hello-world/",
            get_temp_redir("https://words.str4d.xyz/hello-world/"),
        )
        .route(
            "/2011/07/08/the-joys-of-running-your-own-server/",
            get_temp_redir("https://words.str4d.xyz/the-joys-of-running-your-own-server/"),
        )
        .route(
            "/2011/07/25/a-gnulinux-version-of-windows-alt-codes/",
            get_temp_redir("https://words.str4d.xyz/a-linux-version-of-windows-alt-codes/"),
        )
}

pub(crate) fn blog() -> Router {
    Router::new().route("/", get_temp_redir("https://words.str4d.xyz"))
}
