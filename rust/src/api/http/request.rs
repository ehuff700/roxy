use crate::{frb_generated::StreamSink, BackendError};
use flutter_rust_bridge::frb;
use http_body_util::BodyExt;
use hyper::{body::Incoming, header::HOST, Request, Uri};
use hyper_util::{
    client::legacy::{connect::HttpConnector, Client},
    rt::TokioExecutor,
};
use std::fmt::Write;
use std::num::NonZero;
use tokio::sync::oneshot;

#[frb(mirror(StatusCode))]
#[allow(dead_code)]
pub struct _StatusCode(NonZero<u16>);
type RequestChannel = oneshot::Sender<RoxyRequest>;

#[frb]
pub struct RoxyRequest {
    #[frb(skip)]
    response_channel: Option<RequestChannel>,
    inner: hyper::Request<Incoming>,
}

impl RoxyRequest {
    pub fn from(req: Request<hyper::body::Incoming>) -> (Self, oneshot::Receiver<RoxyRequest>) {
        let (tx, rx) = oneshot::channel();
        (
            Self {
                inner: req,
                response_channel: Some(tx),
            },
            rx,
        )
    }
    /// Forwards a request to the target host, returning a response.
    pub async fn forward_request(self) -> Result<RoxyResponse, BackendError> {
        let req = self.inner;
        if let Some(host) = req.headers().get(HOST).and_then(|h| h.to_str().ok()) {
            let mut uri_parts = req.uri().clone().into_parts();
            uri_parts.scheme = Some(unsafe { "http".parse().unwrap_unchecked() });
            uri_parts.authority = Some(host.parse().unwrap());
            uri_parts.path_and_query = uri_parts
                .path_and_query
                .or_else(|| Some("/".parse().unwrap()));
            debug!("uri parts: {:?}", uri_parts);
            let new_uri = Uri::from_parts(uri_parts).expect("failed to construct URI from parts");

            let (mut parts, body) = req.into_parts();
            parts.uri = new_uri;
            let new_request = Request::from_parts(parts, body);
            let client =
                Client::builder(TokioExecutor::new()).build::<_, Incoming>(HttpConnector::new());
            debug!("new request: {:?}", new_request);
            Ok(RoxyResponse::from(
                client
                    .request(new_request)
                    .await
                    .map_err(BackendError::ProxyRequest)?,
            ))
        } else {
            Err(BackendError::MissingOrInvalidHostHeader)
        }
    }
}

#[frb]
pub struct RoxyResponse {
    inner: hyper::Response<Incoming>,
}

impl RoxyResponse {
    pub async fn body(&mut self, sink: StreamSink<String>) -> Result<(), BackendError> {
        /* TODO: This is a mess, figure out how to handle streams properly
        while let Some(frame) = self.inner.frame().await {
            match frame {
                Ok(bytes) => {
                    let bytes = bytes.map_data(|d| String::from_utf8_lossy(&d).to_string());
                    if bytes.is_data() {
                        debug!("{:?}", bytes);
                        let _ = sink.add(unsafe { bytes.into_data().unwrap_unchecked() });
                    } else if bytes.is_trailers() {
                        let headers = unsafe { bytes.into_trailers().unwrap_unchecked() };
                        debug!("THESE ARE THE HEADERS: {:?}", headers);
                        let mut trailer_str = String::new();

                        for (key, value) in headers.iter() {
                            let _ = writeln!(
                                trailer_str,
                                "{key}: {}",
                                String::from_utf8_lossy(value.as_bytes())
                            );
                        }
                        let _ = sink.add(trailer_str);
                    }
                }
                Err(why) => {
                    let _ = sink.add_error(BackendError::BodyProcessing(why));
                }
            }
        }*/
        Ok(())
    }

    #[frb(sync, getter)]
    pub fn method(&self) -> _StatusCode {
        _StatusCode(unsafe { NonZero::new_unchecked(self.inner.status().as_u16()) })
    }
}

impl From<hyper::Response<Incoming>> for RoxyResponse {
    fn from(inner: hyper::Response<Incoming>) -> Self {
        Self { inner }
    }
}
