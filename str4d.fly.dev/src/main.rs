use tower_http::trace::TraceLayer;
use tracing_subscriber::fmt::format::FmtSpan;

mod str4d_xyz;

#[tokio::main]
async fn main() {
    // Filter traces based on the RUST_LOG env var, or, if it's not set,
    // default to show info-level details.
    let filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "axum=info,str4d-fly-dev=info,tower_http=info,tracing=info".to_owned());

    tracing_subscriber::fmt()
        // Use the filter we built above to determine which traces to record.
        .with_env_filter(filter)
        // Record an event when each span closes. This can be used to time our
        // routes' durations!
        .with_span_events(FmtSpan::CLOSE)
        .init();

    let app = str4d_xyz::build().layer(TraceLayer::new_for_http());

    // IPv6 + IPv6 any addr
    let addr = ([0, 0, 0, 0, 0, 0, 0, 0], 8080).into();
    tracing::debug!("Listening on {}", addr);
    let server = axum::Server::bind(&addr).serve(app.into_make_service());
    if let Err(e) = server.await {
        tracing::error!("Server error: {}", e);
    }
}
