//! End-to-end behavior tests.

use altair_otel::{Config, Exporter};

#[tokio::test]
async fn init_with_none_exporter_succeeds() {
    // This test only checks initialization wires together — no exporter actually fires.
    // Subsequent calls will return AlreadyInitialized; we only verify the first call.
    let cfg = Config::builder()
        .service_name("test-svc")
        .exporter(Exporter::None)
        .build();
    let r = cfg.init();
    // Either Ok (first call) or AlreadyInitialized (if a previous test ran first in this process).
    assert!(r.is_ok() || matches!(r, Err(altair_otel::Error::AlreadyInitialized)));
}
