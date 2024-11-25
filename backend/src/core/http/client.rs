use hyper::{
    body::Incoming,
    header::{Entry, COOKIE, HOST},
    Request,
};
use hyper_rustls::{ConfigBuilderExt, HttpsConnectorBuilder};
use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};

use crate::{
    api::http::request::{RoxyRequest, RoxyResponse},
    BackendError,
};

/// An HTTP client that supports both secure (HTTPS) and non-secure (HTTP) connections.
#[derive(Debug, Clone)]
pub struct HttpClient {
    /// TLS configuration for HTTPS connections. None if using HTTP.
    tls_config: Option<rustls::ClientConfig>,
}

/// A trait for HTTP clients that can handle both secure (HTTPS) and non-secure (HTTP) requests.
///
/// This trait provides a common interface for different HTTP client implementations,
/// allowing them to handle both HTTP and HTTPS requests with appropriate TLS configuration.
pub trait HttpClientExt {
    /// Creates a new client instance with the specified security setting.
    fn new(secure: bool) -> Result<Self, BackendError>
    where
        Self: Sized;

    /// Sends an HTTP request without TLS.
    async fn send_http(&self, req: RoxyRequest) -> Result<RoxyResponse, BackendError>;

    /// Sends an HTTPS request using the provided TLS configuration.
    async fn send_https(
        &self,
        req: RoxyRequest,
        tls_config: &rustls::ClientConfig,
    ) -> Result<RoxyResponse, BackendError>;

    /// Sends a request using either HTTP or HTTPS based on the client's TLS configuration.
    async fn send(&self, req: RoxyRequest) -> Result<RoxyResponse, BackendError> {
        match self.tls_config() {
            Some(tls_config) => self.send_https(req, tls_config).await,
            None => self.send_http(req).await,
        }
    }

    /// Returns the client's TLS configuration if HTTPS is enabled.
    fn tls_config(&self) -> Option<&rustls::ClientConfig>;
}

impl HttpClientExt for HttpClient {
    fn new(secure: bool) -> Result<Self, BackendError> {
        let tls_config = match secure {
            true => Some(
                rustls::ClientConfig::builder()
                    .with_native_roots()
                    .map_err(BackendError::TlsSetupError)?
                    .with_no_client_auth(),
            ),
            false => None,
        };
        Ok(Self { tls_config })
    }

    async fn send_http(&self, req: RoxyRequest) -> Result<RoxyResponse, BackendError> {
        let mut http = HttpConnector::new();
        http.enforce_http(true);
        self.send_with_client(req, |builder| builder.build(http))
            .await
    }

    async fn send_https(
        &self,
        req: RoxyRequest,
        tls_config: &rustls::ClientConfig,
    ) -> Result<RoxyResponse, BackendError> {
        let https = HttpsConnectorBuilder::new()
            .with_tls_config(tls_config.clone())
            .https_or_http()
            .enable_all_versions()
            .build();
        self.send_with_client(req, |builder| builder.build(https))
            .await
    }

    fn tls_config(&self) -> Option<&rustls::ClientConfig> {
        self.tls_config.as_ref()
    }
}

impl HttpClient {
    async fn send_with_client<F, C>(
        &self,
        req: RoxyRequest,
        build_client: F,
    ) -> Result<RoxyResponse, BackendError>
    where
        F: FnOnce(
            &mut hyper_util::client::legacy::Builder,
        ) -> hyper_util::client::legacy::Client<C, Incoming>,
        C: hyper_util::client::legacy::connect::Connect + Clone + Send + Sync + 'static,
    {
        let (request_id, hyper_req) = req.deconstruct();
        let hyper_req = self.sanitize_request(hyper_req);
        trace!("REQUEST: forwarding request... {hyper_req:?}");
        let client = build_client(
            hyper_util::client::legacy::Client::builder(TokioExecutor::new())
                .http1_title_case_headers(true)
                .http1_preserve_header_case(true),
        );

        let response = match client.request(hyper_req).await {
            Ok(res) => RoxyResponse::new(res, request_id),
            Err(why) => {
                error!("REQUEST: error forwarding request: {why}");
                RoxyResponse::error(request_id)
            }
        };

        Ok(response)
    }

    fn sanitize_request(&self, mut req: Request<Incoming>) -> Request<Incoming> {
        req.headers_mut().remove(HOST);
        if let Entry::Occupied(mut cookies) = req.headers_mut().entry(COOKIE) {
            let joined_cookies = bstr::join(b"; ", cookies.iter());
            cookies.insert(joined_cookies.try_into().unwrap());
        }
        req
    }
}
