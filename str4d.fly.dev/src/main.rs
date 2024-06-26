use std::net::{IpAddr, Ipv6Addr};

use axum::ServiceExt;
use metrics_exporter_prometheus::PrometheusBuilder;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing_subscriber::fmt::format::FmtSpan;

mod util;

mod atp_fyi;
mod cryptography_design;
mod jackgrigg_com;
mod rfc_observer;
mod siso_dev;
mod sssh;
mod str4d_xyz;

#[tokio::main]
async fn main() {
    println!("Printing something as early as possible so fly.io sees it.");

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
        .add("www.jackgrigg.com", ["jackgrigg.com"], jackgrigg_com::www())
        .handle("blog.jackgrigg.com", jackgrigg_com::blog())
        .add("str4d.xyz", ["www.str4d.xyz"], str4d_xyz::build())
        .add("siso.dev", ["www.siso.dev"], siso_dev::build())
        .add(
            "cryptography.design",
            ["www.cryptography.design"],
            cryptography_design::build(),
        )
        .add("atp.fyi", ["www.atp.fyi"], atp_fyi::build())
        .add("s-s.sh", ["www.s-s.sh"], sssh::build())
        .add("rfc.observer", ["www.rfc.observer"], rfc_observer::build())
        .handle("rust.rfc.observer", rfc_observer::rust::build())
        .layer(util::MetricsLayer::new())
        .layer(TraceLayer::new_for_http());

    let addr: (IpAddr, _) = (Ipv6Addr::UNSPECIFIED.into(), 8080);
    tracing::debug!("Listening on {:?}", addr);

    match TcpListener::bind(addr).await {
        Err(e) => tracing::error!("Failed to bind to listening address: {}", e),
        Ok(listener) => {
            let server = axum::serve(listener, app.into_make_service());
            if let Err(e) = server.await {
                tracing::error!("Server error: {}", e);
            }
        }
    }
}
