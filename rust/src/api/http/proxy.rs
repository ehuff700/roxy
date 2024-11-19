use crate::{frb_generated::StreamSink, BackendError};

use flutter_rust_bridge::frb;
use http_body_util::Full;
use hyper::{body::Bytes, server::conn::http1, service::service_fn, Request, Response};
use hyper_util::rt::TokioIo;
use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::Arc,
};
use tokio::{net::TcpListener, sync::Mutex};

use super::request::RoxyRequest;

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

    /// Starts the proxy server.
    pub async fn start_server(&self, sink: StreamSink<RoxyRequest>) -> Result<(), BackendError> {
        let sink = Arc::new(Mutex::new(sink));
        let listener = TcpListener::bind(self.laddr)
            .await
            .map_err(BackendError::ProxySetup)?;

        tokio::task::spawn(async move {
            while let Ok((stream, addr)) = listener.accept().await {
                let io = TokioIo::new(stream);
                let sink = Arc::clone(&sink);

                let service = service_fn(move |req: Request<hyper::body::Incoming>| {
                    let sink = Arc::clone(&sink);

                    async move {
                        let (request, rx) = RoxyRequest::from(req);
                        let _ = sink.lock().await.add(request);
                        let modified_request = rx.await.unwrap();
                        let response = modified_request.forward_request().await?;
                        
                        Ok::<_, hyper::Error>(Response::new(Full::new(Bytes::from(
                            "Hello, World!",
                        ))))
                    }
                });

                tokio::task::spawn(async move {
                    if let Err(why) = http1::Builder::new().serve_connection(io, service).await {
                        eprintln!("Error serving request on {}: {}", addr, why);
                    }
                });
            }
        });
        Ok(())
    }
}
