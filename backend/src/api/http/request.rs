use crate::{api::utils::error::IntoResponseError, frb_generated::StreamSink, BackendError};
use flutter_rust_bridge::frb;
use http_body_util::{combinators::BoxBody, BodyExt, Full};
use hyper::{
    body::{Body, Bytes, Incoming},
    Request, Response,
};
use hyper_util::rt::TokioExecutor;
use std::num::NonZero;

use hyper::http::response::Parts;

#[frb(mirror(hyper::StatusCode))]
#[allow(dead_code)]
pub struct _StatusCode(NonZero<u16>);

#[frb]
pub struct RoxyRequest {
    #[frb(ignore)]
    inner: hyper::Request<Incoming>,
    request_id: u64,
}

impl RoxyRequest {
    #[frb(ignore)]
    pub fn new(inner: Request<hyper::body::Incoming>, request_id: u64) -> Self {
        Self { inner, request_id }
    }

    /// Forwards a request to the target host, returning a response.
    pub async fn forward_request(self) -> Result<RoxyResponse, BackendError> {
        let mut req = self.inner;
        req.headers_mut().remove("Accept-Encoding"); // TODO: support encoding
        req.headers_mut().remove("If-Modified-Since");
        req.headers_mut().remove("If-None-Match");

        let client = hyper_util::client::legacy::Client::builder(TokioExecutor::new()).build_http();
        trace!("FORWARDING REQUEST: {:#?}", req);
        let response = client
            .request(req)
            .await
            .map_err(BackendError::ProxyRequest)?;

        Ok(RoxyResponse::new(response, self.request_id))
    }
}

enum BodyType {
    Incoming(Incoming),
    Full(Full<Bytes>),
}

impl BodyType {
    fn as_incoming_mut(&mut self) -> Option<&mut Incoming> {
        match self {
            BodyType::Incoming(body) => Some(body),
            _ => None,
        }
    }

    fn as_full_mut(&mut self) -> Option<&mut Full<Bytes>> {
        match self {
            BodyType::Full(body) => Some(body),
            _ => None,
        }
    }
}

#[frb]
pub struct RoxyResponse {
    #[frb(ignore)]
    body_type: BodyType,
    #[frb(ignore)]
    parts: Parts,
    request_id: u64,
}

impl RoxyResponse {
    #[frb(ignore)]
    pub fn new(inner: hyper::Response<Incoming>, request_id: u64) -> Self {
        trace!("CREATING RESPONSE: {:#?}", inner);
        let (parts, body) = inner.into_parts();
        let body_type = BodyType::Incoming(body);
        Self {
            parts,
            request_id,
            body_type,
        }
    }

    /// Processes the body of the response.
    ///
    /// # Safety
    /// This function is unsafe because it assumes that the body is an `Incoming` body. It is the caller's responsibility to ensure this is the case.
    async unsafe fn process_incoming_body(
        &mut self,
        sink: StreamSink<String>,
    ) -> Result<(), BackendError> {
        let body = unsafe { self.body_type.as_incoming_mut().unwrap_unchecked() };
        let mut body_bytes = Vec::with_capacity(body.size_hint().lower() as _);
        while let Some(frame) = body.map_err(BackendError::BodyProcessing).frame().await {
            let frame = frame?;
            if let Some(chunk) = frame.data_ref() {
                body_bytes.extend_from_slice(chunk);
                let string = String::from_utf8(chunk.to_vec())
                    .unwrap_or(String::from("Invalid Utf-8 Sequence."));
                let _ = sink.add(string);
            }
        }
        self.body_type = BodyType::Full(Full::from(Bytes::from(body_bytes)));
        Ok(())
    }

    /// Processes the body of the response.
    ///
    /// # Safety
    /// This function is unsafe because it assumes that the body is a `Full` body. It is the caller's responsibility to ensure this is the case.
    async unsafe fn process_full_body(
        &mut self,
        sink: StreamSink<String>,
    ) -> Result<(), BackendError> {
        let body = unsafe { self.body_type.as_full_mut().unwrap_unchecked() };
        while let Some(Ok(frame)) = body.frame().await {
            if let Some(chunk) = frame.data_ref() {
                let string = String::from_utf8(chunk.to_vec())
                    .unwrap_or_else(|_| String::from("Invalid Utf-8 Sequence."));
                let _ = sink.add(string);
            }
        }
        Ok(())
    }

    /// Processes the body of the response.
    ///
    /// If the body member is none, the process_body_none function will be called to stream the body back to the client.
    pub async fn body(&mut self, sink: StreamSink<String>) -> Result<(), BackendError> {
        match self.body_type {
            BodyType::Incoming(_) => unsafe { self.process_incoming_body(sink).await? },
            BodyType::Full(_) => unsafe { self.process_full_body(sink).await? },
        };
        Ok(())
    }

    #[frb(sync, getter)]
    pub fn status_code(&self) -> _StatusCode {
        _StatusCode(unsafe { NonZero::new_unchecked(self.parts.status.as_u16()) })
    }

    #[frb(sync, getter)]
    pub fn request_id(&self) -> u64 {
        self.request_id
    }

    #[frb(ignore)]
    /// Constructs an error response.
    pub fn error(request_id: u64) -> Self {
        // Safety: We know that the body is a `Full` body because we are creating an error response.
        let response = unsafe {
            Response::builder()
                .status(500)
                .body(Full::new(Bytes::from_static(
                    b"Error rendering response. See Debug logs for more information",
                )))
                .unwrap_unchecked()
        };

        let (parts, body) = response.into_parts();
        let body_type = BodyType::Full(body);
        Self {
            parts,
            request_id,
            body_type,
        }
    }
    /// Converts the response back into a hyper response.
    #[frb(ignore)]
    pub fn into_response(self) -> hyper::Response<BoxBody<Bytes, IntoResponseError>> {
        match self.body_type {
            BodyType::Incoming(body) => {
                let body = body.map_err(IntoResponseError::BodyError).boxed();
                hyper::Response::from_parts(self.parts, body)
            }
            BodyType::Full(body) => hyper::Response::from_parts(
                self.parts,
                body.map_err(|_| IntoResponseError::Infallible).boxed(),
            ),
        }
    }
}
