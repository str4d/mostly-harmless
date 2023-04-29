use std::convert::Infallible;

use hyper::service::make_service_fn;
use metrics_exporter_prometheus::PrometheusBuilder;
use tower_http::trace::TraceLayer;
use tracing_subscriber::fmt::format::FmtSpan;

mod util;

mod cryptography_design;
mod siso_dev;
mod sssh;
mod str4d_xyz;

#[tokio::main]
async fn main() {
    println!("Starting str4d.fly.dev binary");

    // Filter traces based on the RUST_LOG env var, or, if it's not set,
    // default to show info-level details.
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_owned());

    tracing_subscriber::fmt()
        // Use the filter we built above to determine which traces to record.
        .with_env_filter(filter)
        // Record an event when each span closes. This can be used to time our
        // routes' durations!
        .with_span_events(FmtSpan::CLOSE)
        .init();

    tracing::info!("Starting metrics");
    if let Err(e) = PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0, 0, 0, 0, 0], 9091))
        .install()
    {
        tracing::error!("Failed to install metrics server: {}", e);
    };

    tracing::info!("Starting server");
    let app = util::Multiplexer::new()
        .redirect("www.str4d.xyz", "https://str4d.xyz")
        .handle("str4d.xyz", str4d_xyz::build())
        .redirect_temporary("www.siso.dev", "https://siso.dev")
        .handle("siso.dev", siso_dev::build())
        .redirect_temporary("www.cryptography.design", "https://cryptography.design")
        .handle("cryptography.design", cryptography_design::build())
        .redirect_temporary("www.s-s.sh", "https://s-s.sh")
        .handle("s-s.sh", sssh::build())
        .layer(util::MetricsLayer::new())
        .layer(TraceLayer::new_for_http());

    // IPv6 + IPv6 any addr
    let addr = ([0, 0, 0, 0, 0, 0, 0, 0], 8080).into();
    tracing::debug!("Listening on {}", addr);
    let server = axum::Server::bind(&addr).serve(make_service_fn(move |_| {
        let app = app.clone();
        async move { Ok::<_, Infallible>(app) }
    }));
    if let Err(e) = server.await {
        tracing::error!("Server error: {}", e);
    }
}
