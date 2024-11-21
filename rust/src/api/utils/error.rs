use flutter_rust_bridge::frb;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BackendError {
    #[error("Failed to setup proxy server.")]
    ProxySetup(#[source] tokio::io::Error),
    #[error("Missing host header in the request. Please ensure the 'Host' header is set in the request.")]
    MissingOrInvalidHostHeader,
    #[error("Failed to execute the proxy request.")]
    ProxyRequest(#[source] hyper_util::client::legacy::Error),
    #[error("Failed to process the response body.")]
    BodyProcessing(#[source] hyper::Error),
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
