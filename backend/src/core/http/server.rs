use crate::{
    api::http::{
        request::{RoxyRequest, RoxyResponse},
        server::ProxyConfig,
    },
    core::http::client::HttpClientExt,
    tls_extras::{CERT, KEY},
    BackendError,
};

use flutter_rust_bridge::DartFnFuture;
use hyper::{body::Incoming, server::conn::http1, service::service_fn, Request};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto,
};
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer},
    ServerConfig,
};
use std::{
    fs::File,
    io::BufReader,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
    sync::{atomic::AtomicU64, Arc, Once},
};
use tokio::net::TcpListener;
use tokio_rustls::TlsAcceptor;

use super::client::HttpClient;
pub static REQ_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

type OnRequestCallback =
    Arc<dyn Fn(RoxyRequest) -> DartFnFuture<RoxyRequest> + Send + Sync + 'static>;
type OnResponseCallback =
    Arc<dyn Fn(RoxyResponse) -> DartFnFuture<RoxyResponse> + Send + Sync + 'static>;

pub struct CoreProxyServer {
    config: ProxyConfig,
    certs: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
    client: Arc<HttpClient>,
}

impl CoreProxyServer {
    pub fn new(config: ProxyConfig) -> Result<Self, BackendError> {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        });
        let client = Arc::new(HttpClient::new(true)?);
        let certs = Self::load_tls_certs(config.cert_path.as_deref())?;
        let key = Self::load_private_key(config.key_path.as_deref())?;

        Ok(Self {
            config,
            certs,
            key,
            client,
        })
    }

    pub async fn process_connection(
        http_client: Arc<HttpClient>,
        req: Request<Incoming>,
        on_request: OnRequestCallback,
        on_response: OnResponseCallback,
    ) -> Result<RoxyResponse, BackendError> {
        let request = RoxyRequest::new(req);
        let modified_request = on_request(request).await;
        let roxy_response = http_client.send(modified_request).await?;
        let final_response = on_response(roxy_response).await;
        Ok(final_response)
    }

    pub async fn setup_https(
        extra: (
            Vec<CertificateDer<'static>>,
            PrivateKeyDer<'static>,
            ProxyConfig,
        ),
        https_client: Arc<HttpClient>,
        on_request: OnRequestCallback,
        on_response: OnResponseCallback,
    ) -> Result<(), BackendError> {
        let (certs, key, config) = extra;
        let mut server_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(BackendError::TlsConfigSetup)?;
        server_config.alpn_protocols =
            vec![b"h2".to_vec(), b"http/1.1".to_vec(), b"http/1.0".to_vec()];
        let tls_acceptor = TlsAcceptor::from(Arc::new(server_config));
        let listener = TcpListener::bind(format!("{}:{}", config.ip, config.https_port))
            .await
            .map_err(BackendError::ProxySetup)?;

        tokio::task::spawn({
            let client = Arc::clone(&https_client);
            async move {
                loop {
                    let service = service_fn({
                        let client = Arc::clone(&client);
                        let on_request = Arc::clone(&on_request);
                        let on_response = Arc::clone(&on_response);
                        move |req| {
                            let client = Arc::clone(&client);
                            let on_request = Arc::clone(&on_request);
                            let on_response = Arc::clone(&on_response);
                            async move {
                                Self::process_connection(client, req, on_request, on_response)
                                    .await
                                    .map(|r| r.into_response())
                            }
                        }
                    });

                    let (stream, addr) = listener.accept().await.unwrap(); // TODO: handle error

                    tokio::task::spawn({
                        let tls_acceptor = tls_acceptor.clone();
                        async move {
                            let tls_stream = match tls_acceptor.accept(stream).await {
                                Ok(tls_stream) => tls_stream,
                                Err(e) => {
                                    error!("error accepting TLS stream: {e}");
                                    return;
                                }
                            };
                            auto::Builder::new(TokioExecutor::new())
                                .serve_connection(TokioIo::new(tls_stream), service)
                                .await
                                .inspect_err(|e| {
                                    error!("error serving connection on {}: {e}", addr)
                                });
                        }
                    });
                }
            }
        });
        Ok(())
    }

    pub async fn setup_http(
        config: ProxyConfig,
        http_client: Arc<HttpClient>,
        on_request: OnRequestCallback,
        on_response: OnResponseCallback,
    ) -> Result<(), BackendError> {
        let listener = TcpListener::bind(format!("{}:{}", config.ip, config.http_port))
            .await
            .map_err(BackendError::ProxySetup)?;

        tokio::task::spawn({
            let client = Arc::clone(&http_client);
            async move {
                while let Ok((stream, addr)) = listener.accept().await {
                    let service = service_fn({
                        let client = Arc::clone(&client);
                        let on_request = Arc::clone(&on_request);
                        let on_response = Arc::clone(&on_response);
                        move |req| {
                            let client = Arc::clone(&client);
                            let on_request = Arc::clone(&on_request);
                            let on_response = Arc::clone(&on_response);
                            async move {
                                Self::process_connection(client, req, on_request, on_response)
                                    .await
                                    .map(|r| r.into_response())
                            }
                        }
                    });

                    tokio::task::spawn(async move {
                        if let Err(why) = http1::Builder::new()
                            .title_case_headers(true)
                            .preserve_header_case(true)
                            .serve_connection(TokioIo::new(stream), service)
                            .await
                        {
                            error!("error serving connection on {}: {why}", addr)
                        }
                    });
                }
            }
        });

        Ok(())
    }

    /// Listens for incoming HTTP requests, forwards them to dart for optional modification, and then returns the response
    pub async fn proxy_request(
        &self,
        on_request: impl Fn(RoxyRequest) -> DartFnFuture<RoxyRequest> + Send + Sync + 'static,
        on_response: impl Fn(RoxyResponse) -> DartFnFuture<RoxyResponse> + Send + Sync + 'static,
    ) -> Result<(), BackendError> {
        let on_request = Arc::new(on_request) as OnRequestCallback;
        let on_response = Arc::new(on_response) as OnResponseCallback;
        let http_client = &self.client;

        tokio::task::spawn(Self::setup_http(
            self.config.clone(),
            Arc::clone(&http_client),
            Arc::clone(&on_request),
            Arc::clone(&on_response),
        ));

        let extra = (
            self.certs.clone(),
            self.key.clone_key(),
            self.config.clone(),
        );

        tokio::task::spawn(Self::setup_https(
            extra,
            Arc::clone(&http_client),
            Arc::clone(&on_request),
            Arc::clone(&on_response),
        ));

        Ok(())
    }

    /// Loads the TLS certificates from the given path or the default certificate if no path is provided
    pub fn load_tls_certs(
        cert_path: Option<&str>,
    ) -> Result<Vec<CertificateDer<'static>>, BackendError> {
        let certs = if let Some(cert_path) = cert_path {
            let cert = File::open(cert_path).map_err(BackendError::TlsSetupError)?;
            let mut reader = BufReader::new(cert);
            let certs = rustls_pemfile::certs(&mut reader)
                .map(|r| r.map_err(BackendError::TlsSetupError))
                .collect::<Result<Vec<_>, _>>()?;
            certs
        } else {
            let mut reader = CERT.as_ref();
            let certs = rustls_pemfile::certs(&mut reader)
                .map(|r| r.map_err(BackendError::TlsSetupError))
                .collect::<Result<Vec<_>, _>>()?;
            certs
        };
        Ok(certs)
    }

    /// Loads the TLS private key from the given path or the default private key if no path is provided
    pub fn load_private_key(
        key_path: Option<&str>,
    ) -> Result<PrivateKeyDer<'static>, BackendError> {
        let key = if let Some(key_path) = key_path {
            let key = File::open(key_path).map_err(BackendError::TlsSetupError)?;
            let mut reader = BufReader::new(key);
            let keys = rustls_pemfile::private_key(&mut reader)
                .map_err(BackendError::TlsSetupError)?
                .ok_or(BackendError::TlsSetupError(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "No private key found",
                )))?;
            keys
        } else {
            let mut reader = KEY.as_ref();
            let keys = rustls_pemfile::private_key(&mut reader)
                .map_err(BackendError::TlsSetupError)?
                .ok_or(BackendError::TlsSetupError(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "No private key found",
                )))?;
            keys
        };
        Ok(key)
    }
}
