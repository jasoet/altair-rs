//! Client factory.

use temporalio_client::{
    Client as SdkClient, ClientOptions, ClientTlsOptions, Connection, ConnectionOptions, TlsOptions,
};
use url::Url;

use crate::config::{Config, TlsConfig};
use crate::error::{Error, Result};

/// Namespace-level factory for Temporal client construction.
///
/// # Examples
///
/// ```no_run
/// # use altair_temporal::{Client, Config};
/// # async fn example() -> altair_temporal::Result<()> {
/// let cfg = Config { task_queue: "my-queue".into(), ..Default::default() };
/// let client = Client::from_config(&cfg).await?;
/// # Ok(())
/// # }
/// ```
pub struct Client;

impl Client {
    /// Connect using the given configuration.
    ///
    /// Reads any TLS files from disk and attaches them to the connection
    /// before opening the gRPC channel.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Configuration`] if the host URL is invalid or TLS
    /// files cannot be read.  Returns [`Error::Connect`] if the gRPC
    /// channel cannot be established.
    pub async fn from_config(cfg: &Config) -> Result<SdkClient> {
        let url = Url::parse(&cfg.host)
            .map_err(|e| Error::Configuration(format!("invalid host: {e}")))?;

        // Build TLS options before starting the builder (bon state machine
        // can't be conditionally branched after `.identity()`).
        let tls_opts = cfg.tls.as_ref().map(build_tls).transpose()?;

        let conn_opts = {
            let b = ConnectionOptions::new(url).identity(cfg.identity.clone());
            if let Some(t) = tls_opts {
                b.tls_options(t).build()
            } else {
                b.build()
            }
        };

        let connection = Connection::connect(conn_opts)
            .await
            .map_err(|e| Error::connect(cfg.host.clone(), Box::new(e) as crate::error::BoxError))?;

        let client_opts = ClientOptions::new(cfg.namespace.clone()).build();

        SdkClient::new(connection, client_opts)
            .map_err(|e| Error::client(Box::new(e) as crate::error::BoxError))
    }
}

fn build_tls(cfg: &TlsConfig) -> Result<TlsOptions> {
    let ca = std::fs::read(&cfg.server_root_ca_cert).map_err(|e| {
        Error::Configuration(format!(
            "read server_root_ca_cert ({}): {e}",
            cfg.server_root_ca_cert.display()
        ))
    })?;

    let client_tls = match (&cfg.client_cert, &cfg.client_key) {
        (None, None) => None,
        (Some(cert_path), Some(key_path)) => {
            let cert = std::fs::read(cert_path).map_err(|e| {
                Error::Configuration(format!("read client_cert ({}): {e}", cert_path.display()))
            })?;
            let key = std::fs::read(key_path).map_err(|e| {
                Error::Configuration(format!("read client_key ({}): {e}", key_path.display()))
            })?;
            Some(ClientTlsOptions {
                client_cert: cert,
                client_private_key: key,
            })
        }
        _ => {
            return Err(Error::Configuration(
                "client_cert and client_key must both be set or both unset".to_string(),
            ));
        }
    };

    Ok(TlsOptions {
        server_root_ca_cert: Some(ca),
        domain: cfg.server_name_override.clone(),
        client_tls_options: client_tls,
    })
}
