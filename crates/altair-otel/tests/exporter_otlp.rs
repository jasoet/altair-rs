//! Verify the OTLP exporter build path (doesn't actually export — exporter just needs to build).

use altair_otel::{Config, Exporter};

#[tokio::test]
async fn init_with_otlp_exporter_builds() {
    // OTLP exporter will try to talk to localhost:4317 in the background;
    // we don't care if it succeeds — only that init() wires things together.
    let cfg = Config::builder()
        .service_name("otlp-test")
        .otlp_endpoint("http://127.0.0.1:9999")
        .exporter(Exporter::Otlp)
        .build();
    let r = cfg.init();
    assert!(
        r.is_ok() || matches!(r, Err(altair_otel::Error::AlreadyInitialized)),
        "OTLP init must build cleanly: got {r:?}",
    );
}
