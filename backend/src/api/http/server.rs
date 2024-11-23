use std::path::PathBuf;

use crate::{core::http::server::CoreProxyServer, BackendError};

use flutter_rust_bridge::{frb, DartFnFuture};

use super::request::{RoxyRequest, RoxyResponse};

/// Configuration options for the proxy server
#[derive(Clone)]
pub struct ProxyConfig {
    /// The IP address the proxy server will listen on
    pub ip: String,
    /// The port number the proxy server will listen on for HTTP requests
    pub http_port: u16,
    /// The port number the proxy server will listen on for HTTPS requests
    pub https_port: u16,
    /// The path to the certificate file for HTTPS requests
    pub cert_path: Option<String>,
    /// The path to the private key file for HTTPS requests
    pub key_path: Option<String>,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            ip: "127.0.0.1".to_string(),
            http_port: 5280,
            https_port: 5281,
            cert_path: None,
            key_path: None,
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
        &self,
        on_request: impl Fn(RoxyRequest) -> DartFnFuture<RoxyRequest> + Send + Sync + 'static,
        on_response: impl Fn(RoxyResponse) -> DartFnFuture<RoxyResponse> + Send + Sync + 'static,
    ) -> Result<(), BackendError> {
        self.core.proxy_request(on_request, on_response).await
    }
}
