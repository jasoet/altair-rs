//! Subscriber + provider wire-up.

use crate::config::{Config, Exporter, LogFormat};
use crate::error::{Error, Result};
use opentelemetry::KeyValue;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub(crate) fn init(config: &Config) -> Result<()> {
    opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());

    let resource = build_resource(config);
    let provider = build_tracer_provider(config, resource)?;
    opentelemetry::global::set_tracer_provider(provider.clone());

    let tracer = provider.tracer("altair");
    let otel_layer = tracing_opentelemetry::layer().with_tracer(tracer);
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(otel_layer);

    let try_init = match config.log_format {
        LogFormat::Json => registry
            .with(tracing_subscriber::fmt::layer().json())
            .try_init(),
        LogFormat::Pretty => registry
            .with(tracing_subscriber::fmt::layer().pretty())
            .try_init(),
    };
    try_init.map_err(|_| Error::AlreadyInitialized)?;

    Ok(())
}

fn build_resource(config: &Config) -> Resource {
    let mut attrs = vec![KeyValue::new("service.name", config.service_name.clone())];
    if let Some(v) = &config.service_version {
        attrs.push(KeyValue::new("service.version", v.clone()));
    }
    for (k, v) in &config.resource_attributes {
        attrs.push(KeyValue::new(k.clone(), v.clone()));
    }
    Resource::builder().with_attributes(attrs).build()
}

fn build_tracer_provider(config: &Config, resource: Resource) -> Result<SdkTracerProvider> {
    let builder = SdkTracerProvider::builder().with_resource(resource);

    let provider = match config.exporter {
        Exporter::Otlp => {
            let mut exporter_builder = opentelemetry_otlp::SpanExporter::builder().with_tonic();
            if let Some(endpoint) = &config.otlp_endpoint {
                exporter_builder = exporter_builder.with_endpoint(endpoint);
            }
            let exporter = exporter_builder
                .build()
                .map_err(|e| Error::Exporter(e.to_string()))?;
            builder.with_batch_exporter(exporter).build()
        }
        Exporter::Stdout => {
            let exporter = opentelemetry_stdout::SpanExporter::default();
            builder.with_simple_exporter(exporter).build()
        }
        Exporter::None => builder.build(),
    };

    Ok(provider)
}
