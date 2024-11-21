use crate::BackendError;

use flutter_rust_bridge::{frb, DartFnFuture};
use hyper::{body::Incoming, server::conn::http1, service::service_fn, Request};
use hyper_util::rt::TokioIo;
use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::{atomic::AtomicU64, Arc},
};
use tokio::net::TcpListener;

use super::request::{RoxyRequest, RoxyResponse};

#[frb(ignore)]
pub static REQ_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

#[frb]
pub struct ProxyServer {
    laddr: SocketAddr,
}

impl ProxyServer {
    #[frb(sync)]
    pub fn new(ip: String, port: u16) -> Self {
        let ip = IpAddr::from_str(&ip).unwrap_or(IpAddr::from_str("127.0.0.1").unwrap());
        let laddr = SocketAddr::new(ip, port);
        ProxyServer { laddr }
    }

    /// Listens for incoming HTTP requests, forwards them to dart for optional modification, and then returns the response
    pub async fn proxy_request(
        &self,
        on_request: impl Fn(RoxyRequest) -> DartFnFuture<RoxyRequest> + Send + Sync + 'static,
        on_response: impl Fn(RoxyResponse) -> DartFnFuture<RoxyResponse> + Send + Sync + 'static,
    ) -> Result<(), BackendError> {
        let listener = TcpListener::bind(self.laddr)
            .await
            .map_err(BackendError::ProxySetup)?;
        let on_req = Arc::new(on_request);
        let on_res = Arc::new(on_response);

        tokio::task::spawn(async move {
            while let Ok((stream, addr)) = listener.accept().await {
                let io = TokioIo::new(stream);
                let on_req = Arc::clone(&on_req);
                let on_res = Arc::clone(&on_res);

                let service = service_fn(move |req: Request<Incoming>| {
                    let on_req = Arc::clone(&on_req);
                    let on_res = Arc::clone(&on_res);
                    async move {
                        let request_id =
                            REQ_ID_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let request = RoxyRequest::new(req, request_id);

                        let modified_request = on_req(request).await;

                        let roxy_response = modified_request
                            .forward_request()
                            .await
                            .inspect_err(|e| error!("Error forwarding request: {}", e))
                            .unwrap_or_else(|_| RoxyResponse::error(request_id));

                        let final_response = on_res(roxy_response).await;
                        Ok::<_, hyper::Error>(final_response.into_response())
                    }
                });

                tokio::task::spawn(async move {
                    if let Err(why) = http1::Builder::new().serve_connection(io, service).await {
                        error!("error serving request on {}: {}", addr, why);
                    }
                });
            }
        });
        Ok(())
    }
}
