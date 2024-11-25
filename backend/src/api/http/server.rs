use std::sync::Arc;

use crate::{core::http::server::CoreProxyServer, BackendError};

use flutter_rust_bridge::DartFnFuture;

use super::request::{RoxyRequest, RoxyResponse};

/// Configuration options for the proxy server
#[derive(Clone)]
#[frb]
pub struct ProxyConfig {
    /// The IP address the proxy server will listen on
    #[frb(non_final)]
    pub ip: String,
    /// The port number the proxy server will listen on for HTTP(s) requests
    #[frb(non_final)]
    pub port: u16,
    /// The path to the certificate file for HTTPS requests
    #[frb(non_final)]
    pub cert_path: Option<String>,
    /// The path to the private key file for HTTPS requests
    #[frb(non_final)]
    pub key_path: Option<String>,
    /// Whether the proxy client should use HTTPS (true) or HTTP (false)
    #[frb(non_final)]
    pub proxy_client_secure: bool,
}

impl Default for ProxyConfig {
    #[frb(sync)]
    fn default() -> Self {
        Self {
            ip: "127.0.0.1".to_string(),
            port: 5280,
            cert_path: None,
            key_path: None,
            proxy_client_secure: true,
        }
    }
}

pub struct ProxyServer {
    core: CoreProxyServer,
}

impl ProxyServer {
    #[frb(sync)]
    pub fn new(config: ProxyConfig) -> Result<Self, BackendError> {
        let core = CoreProxyServer::new(config)?;
        Ok(Self { core })
    }

    /// Listens for incoming HTTP requests, forwards them to dart for optional modification, and then returns the response
    pub async fn proxy_request(
        self,
        on_request: impl Fn(RoxyRequest) -> DartFnFuture<RoxyRequest> + Send + Sync + 'static,
        on_response: impl Fn(RoxyResponse) -> DartFnFuture<RoxyResponse> + Send + Sync + 'static,
    ) -> Result<(), BackendError> {
        self.core
            .start(Arc::new(on_request), Arc::new(on_response))
            .await
    }
}
