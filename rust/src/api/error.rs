use flutter_rust_bridge::frb;
use thiserror::Error;
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
#[frb]
pub enum Error {
    #[error("Failed to setup proxy server.")]
    ProxySetup(#[source] tokio::io::Error),
    #[error("Missing host header in the request. Please ensure the 'Host' header is set in the request.")]
    MissingHostHeader,
    #[error("Failed to execute the proxy request.")]
    ProxyRequest(#[source] hyper_util::client::legacy::Error),
}
