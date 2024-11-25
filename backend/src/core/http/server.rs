use crate::{
    api::{
        http::{
            request::{RoxyRequest, RoxyResponse},
            server::ProxyConfig,
        },
        utils::error::IntoResponseError,
    },
    core::http::client::HttpClientExt,
    BackendError,
};
use flutter_rust_bridge::DartFnFuture;
use http_body_util::combinators::BoxBody;
use hyper::{
    body::{Bytes, Incoming},
    service::service_fn,
    Method, Request, Response,
};
use hyper::{http::uri::Authority, upgrade::Upgraded, Uri};
use hyper::{http::uri::Scheme, Version};
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto,
};
use std::{
    convert::Infallible,
    future::Future,
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::{atomic::AtomicU64, Arc},
};
use tokio::{
    io::AsyncReadExt,
    net::{TcpListener, TcpStream},
};
use tokio_rustls::TlsAcceptor;

use super::{client::HttpClient, rewind::Rewind, tls::TlsCertCache};
pub static REQ_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

type OnRequestCallback =
    Arc<dyn Fn(RoxyRequest) -> DartFnFuture<RoxyRequest> + Send + Sync + 'static>;
type OnResponseCallback =
    Arc<dyn Fn(RoxyResponse) -> DartFnFuture<RoxyResponse> + Send + Sync + 'static>;

#[derive(Clone)]
pub struct Context {
    on_request: OnRequestCallback,
    on_response: OnResponseCallback,
    client_addr: SocketAddr,
    proxy_client: Arc<HttpClient>,
    tls_cert_cache: TlsCertCache,
    server: auto::Builder<TokioExecutor>,
}

pub struct CoreProxyServer {
    config: ProxyConfig,
    client: Arc<HttpClient>,
    tls_cert_cache: TlsCertCache,
}

impl CoreProxyServer {
    pub fn new(config: ProxyConfig) -> Result<Self, BackendError> {
        let client = Arc::new(HttpClient::new(true)?);
        let tls_cert_cache = TlsCertCache::default();
        Ok(Self {
            config,
            client,
            tls_cert_cache,
        })
    }

    pub async fn proxy_unknown(
        authority: &Authority,
        mut upgraded: Rewind<TokioIo<Upgraded>>,
    ) -> Result<(), BackendError> {
        trace!("proxying unknown protocol to {}", authority.as_str());
        let mut server = TcpStream::connect(authority.as_str())
            .await
            .map_err(BackendError::ProxyUnknown)?;
        if let Err(why) = tokio::io::copy_bidirectional(&mut upgraded, &mut server).await {
            error!("error copying request to server: {why}");
        }
        Ok(())
    }

    pub async fn proxy_https(
        ctx: Context,
        authority: &Authority,
        upgraded: Rewind<TokioIo<Upgraded>>,
    ) -> Result<(), BackendError> {
        let server_cfg = ctx.tls_cert_cache.get_or_insert(authority).await;
        let tls_stream = TlsAcceptor::from(server_cfg)
            .accept(upgraded)
            .await
            .map_err(BackendError::TlsStreamError)?;

        if let Err(why) = Self::serve_stream(
            ctx,
            Scheme::HTTPS,
            authority.clone(),
            TokioIo::new(tls_stream),
        )
        .await
        {
            error!("error serving TLS stream: {why}");
        }
        Ok(())
    }

    pub async fn proxy_http(ctx: Context, req: RoxyRequest) -> Result<RoxyResponse, BackendError> {
        let client = ctx.proxy_client.clone();
        let modified_request = (ctx.on_request)(req).await;
        let resp = client.send(modified_request).await?;
        let final_response = (ctx.on_response)(resp).await;
        Ok(final_response)
    }

    /// MITM HTTPs requests
    pub async fn proxy_connect(
        ctx: Context,
        req: RoxyRequest,
    ) -> Result<RoxyResponse, BackendError> {
        let request_id: u64 = req.request_id();
        let authority = req.uri().authority().cloned();
        let authority = authority.ok_or_else(|| BackendError::MissingOrInvalidAuthority)?;
        let fut = async move {
            let req_inner: Request<Incoming> = req.into();
            let mut upgraded = TokioIo::new(
                hyper::upgrade::on(req_inner)
                    .await
                    .map_err(BackendError::UpgradeError)?,
            );

            let mut buf = [0; 2];
            let bytes_read = upgraded
                .read(&mut buf)
                .await
                .map_err(BackendError::ReadFromUpgraded)?;
            let upgraded =
                Rewind::new_buffered(upgraded, Bytes::copy_from_slice(buf[..bytes_read].as_ref()));

            // \x16\x03 is the TLS version, this is a TLS stream
            if buf == *b"\x16\x03" {
                Self::proxy_https(ctx, &authority, upgraded).await?;
            } else {
                trace!("unknown protocol, first two bytes: {:02?}", buf);
                Self::proxy_unknown(&authority, upgraded).await?;
            }

            Ok::<_, BackendError>(())
        };

        // Spawn future to handle connection
        tokio::task::spawn(fut);

        // Return empty response for CONNECT requests
        Ok::<_, BackendError>(RoxyResponse::empty(request_id))
    }

    pub async fn proxy_websocket(
        _ctx: Context,
        req: RoxyRequest,
    ) -> Result<RoxyResponse, BackendError> {
        debug!("proxying websocket request: {:?}", req);
        Ok(RoxyResponse::empty(req.request_id()))
    }

    pub async fn proxy_service(
        ctx: Context,
        req: Request<Incoming>,
    ) -> Result<Response<BoxBody<Bytes, IntoResponseError>>, Infallible> {
        debug!("proxying request: {:?}", req);
        let req = RoxyRequest::new(req);
        let request_id = req.request_id();
        // MITM HTTPs requests
        let resp = if req.method() == Method::CONNECT {
            Self::proxy_connect(ctx, req).await
        } else if hyper_tungstenite::is_upgrade_request(&req) {
            Self::proxy_websocket(ctx, req).await
        } else {
            Self::proxy_http(ctx, req).await
        };
        Ok(match resp {
            Ok(resp) => resp.into_response(),
            Err(why) => {
                error!("error proxying request: {why}");
                RoxyResponse::error(request_id).into_response()
            }
        })
    }

    pub async fn start(
        self,
        on_request: OnRequestCallback,
        on_response: OnResponseCallback,
    ) -> Result<(), BackendError> {
        let server = {
            let mut builder = auto::Builder::new(TokioExecutor::new());
            builder
                .http1()
                .preserve_header_case(true)
                .title_case_headers(true);
            builder.http2().enable_connect_protocol();
            builder
        };

        let ip_addr = IpAddr::from_str(&self.config.ip)
            .map_err(|e| BackendError::IpAddressParse(self.config.ip.clone(), e))?;
        let laddr = SocketAddr::new(ip_addr, self.config.port);
        let listener = TcpListener::bind(&laddr)
            .await
            .map_err(BackendError::ProxySetup)?;

        let tls_cache = self.tls_cert_cache.clone();
        let http_client = Arc::clone(&self.client);
        loop {
            let server = server.clone();

            if let Ok((stream, client_addr)) = listener
                .accept()
                .await
                .inspect_err(|e| error!("error accepting connection on {laddr}: {e}",))
            {
                let ctx = Context {
                    on_request: Arc::clone(&on_request),
                    on_response: Arc::clone(&on_response),
                    client_addr,
                    proxy_client: Arc::clone(&http_client),
                    tls_cert_cache: tls_cache.clone(),
                    server: server.clone(),
                };

                tokio::task::spawn(async move {
                    let service = service_fn(|req| {
                        let ctx = ctx.clone();
                        async move { Self::proxy_service(ctx, req).await }
                    });

                    let conn = server.serve_connection_with_upgrades(TokioIo::new(stream), service);
                    let conn = std::pin::pin!(conn);
                    conn.await.inspect_err(|e| {
                        error!("error serving connection on {}: {e}", client_addr);
                    })
                });
            }
        }
    }

    /// Serve a stream with a custom scheme and authority
    #[allow(clippy::manual_async_fn)] // manual is necessary because this future needs to be Send
    pub fn serve_stream<I>(
        ctx: Context,
        scheme: Scheme,
        authority: Authority,
        stream: I,
    ) -> impl Future<Output = Result<(), BackendError>> + Send
    where
        I: hyper::rt::Read + hyper::rt::Write + Unpin + Send + 'static,
    {
        async move {
            let server = ctx.server.clone();
            let service = service_fn(|mut req| {
                if req.version() == Version::HTTP_10 || req.version() == Version::HTTP_11 {
                    let (mut parts, body) = req.into_parts();
                    parts.uri = {
                        let mut parts = parts.uri.into_parts();
                        parts.scheme = Some(scheme.clone());
                        parts.authority = Some(authority.clone());
                        Uri::from_parts(parts).expect("failed to create URI")
                    };
                    req = Request::from_parts(parts, body);
                }
                let ctx = ctx.clone();
                async move { Self::proxy_service(ctx, req).await }
            });

            server
                .serve_connection_with_upgrades(stream, service)
                .await
                .map_err(BackendError::ServeConnection)?;

            Ok(())
        }
    }
}
