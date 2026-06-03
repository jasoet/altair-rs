//! Verify the Stdout exporter path in init.rs.

use altair_otel::{Config, Exporter};

#[tokio::test]
async fn init_with_stdout_exporter_succeeds() {
    let cfg = Config::builder()
        .service_name("stdout-test")
        .service_version("0.1.0")
        .resource_attribute("env", "test")
        .exporter(Exporter::Stdout)
        .build()
        .unwrap();
    let r = cfg.init();
    assert!(
        r.is_ok() || matches!(r, Err(altair_otel::Error::AlreadyInitialized)),
        "first init must succeed or already-init: got {r:?}",
    );

    // Hit the meter() and shutdown() paths.
    let counter = altair_otel::meter().u64_counter("stdout.test").build();
    counter.add(1, &[]);
    altair_otel::shutdown();
}
