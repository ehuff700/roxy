use flutter_rust_bridge::frb;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BackendError {
    #[error("Failed to setup proxy server.")]
    ProxySetup(#[source] tokio::io::Error),
    #[error("Missing or invalid authority (host) in the request.")]
    MissingOrInvalidAuthority,
    #[error("Failed to execute the proxy request.")]
    ProxyRequest(#[source] hyper_util::client::legacy::Error),
    #[error("Failed to process the response body.")]
    BodyProcessing(#[source] hyper::Error),
    #[error("Failed to setup TLS.")]
    TlsSetupError(#[source] std::io::Error),
    #[error("Failed to setup TLS config.")]
    TlsConfigSetup(#[source] rustls::Error),
    #[error("Failed to parse IP address: {0}")]
    IpAddressParse(String, #[source] std::net::AddrParseError),
    #[error("Failed to upgrade the http request.")]
    UpgradeError(#[source] hyper::Error),
    #[error("Failed to read from upgraded stream.")]
    ReadFromUpgraded(#[source] std::io::Error),
    #[error("Failed to accept TLS stream.")]
    TlsStreamError(#[source] std::io::Error),
    #[error("Failed to serve connection.")]
    ServeConnection(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("Failed to proxy unknown protocol.")]
    ProxyUnknown(#[source] std::io::Error),
}

impl BackendError {
    #[frb(sync)]
    pub fn display(&self) -> String {
        format!("{}", self)
    }
}

#[derive(Debug)]
pub enum IntoResponseError {
    Infallible,
    BodyError(hyper::Error),
}

impl std::error::Error for IntoResponseError {}
impl std::fmt::Display for IntoResponseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IntoResponseError::Infallible => write!(f, "Infallible error"),
            IntoResponseError::BodyError(e) => write!(f, "Body error: {}", e),
        }
    }
}
