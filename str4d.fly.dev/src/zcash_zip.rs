use axum::{Router, extract::Path, response::Redirect, routing::get};
use hyper::StatusCode;

use crate::util::get_temp_redir;

const ZIPS_Z_CASH: &str = "https://zips.z.cash";

pub(crate) fn build() -> Router {
    Router::new()
        .route("/", get_temp_redir(ZIPS_Z_CASH))
        .route("/{number}", get(redirect_to_zip))
}

async fn redirect_to_zip(Path(number): Path<u16>) -> Result<Redirect, StatusCode> {
    if number < 10_000 {
        Ok(Redirect::temporary(&format!(
            "{ZIPS_Z_CASH}/zip-{number:04}"
        )))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}
