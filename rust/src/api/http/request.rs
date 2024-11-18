use std::num::NonZero;

use crate::api::error::Error;
use flutter_rust_bridge::frb;
use hyper::{body::Incoming, header::HOST, Request, StatusCode, Uri};
use hyper_util::{
    client::legacy::{connect::HttpConnector, Client},
    rt::TokioExecutor,
};

#[frb(mirror(StatusCode))]
pub struct _StatusCode(NonZero<u16>);

#[frb]
pub struct RoxyRequest {
    inner: hyper::Request<Incoming>,
}

impl RoxyRequest {
    pub async fn forward_request(self) -> crate::Result<RoxyResponse> {
        let req = self.inner;
        if let Some(host) = req.headers().get(HOST).and_then(|h| h.to_str().ok()) {
            let mut uri_parts = req.uri().clone().into_parts();
            uri_parts.scheme = Some("http".parse().unwrap());
            uri_parts.authority = Some(host.parse().unwrap());
            let new_uri = Uri::from_parts(uri_parts).expect("failed to construct URI from parts");

            let (mut parts, body) = req.into_parts();
            parts.uri = new_uri;
            let new_request = Request::from_parts(parts, body);
            let client =
                Client::builder(TokioExecutor::new()).build::<_, Incoming>(HttpConnector::new());

            Ok(RoxyResponse::from(
                client
                    .request(new_request)
                    .await
                    .map_err(Error::ProxyRequest)?,
            ))
        } else {
            Err(Error::MissingHostHeader)
        }
    }
}

impl From<hyper::Request<Incoming>> for RoxyRequest {
    fn from(inner: hyper::Request<Incoming>) -> Self {
        RoxyRequest { inner }
    }
}

#[frb]
pub struct RoxyResponse {
    inner: hyper::Response<Incoming>,
}

impl RoxyResponse {
    pub fn body(&self) -> &Incoming {
        self.inner.body()
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
