use core::future::Future;
use std::convert::Infallible;
use std::pin::Pin;

pub use lambda_http;
pub use warp;

use lambda_http::http::response::Parts;
use lambda_http::http::HeaderValue;
use lambda_http::{
    lambda_runtime, Adapter, Body as LambdaBody, Error as LambdaError, Request, RequestExt,
    Response, Service,
};
use mime::Mime;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::marker::PhantomData;
use std::str::FromStr;
use std::task::{Context, Poll};
use warp::hyper::Body as WarpBody;

pub type WarpRequest = warp::http::Request<warp::hyper::Body>;
pub type WarpResponse = warp::http::Response<warp::hyper::Body>;

#[derive(thiserror::Error, Debug)]
pub enum WarpAdapterError {
    #[error("This may never occur, it's infallible!!")]
    Infallible(#[from] std::convert::Infallible),
    #[error("Warp error: `{0:#?}`")]
    HyperError(#[from] warp::hyper::Error),
    #[error("Unexpected error: `{0:#?}`")]
    Unexpected(#[from] LambdaError),
}

pub async fn run<'a, S>(service: S) -> Result<(), LambdaError>
where
    S: Service<WarpRequest, Response = WarpResponse, Error = Infallible> + Send + 'a,
    S::Future: Send + 'a,
{
    lambda_runtime::run(Adapter::from(WarpAdapter::new(service))).await
}

#[derive(Clone)]
pub struct WarpAdapter<'a, S>
where
    S: Service<WarpRequest, Response = WarpResponse, Error = Infallible>,
    S::Future: Send + 'a,
{
    warp_service: S,
    _phantom_data: PhantomData<&'a WarpResponse>,
}

impl<'a, S> WarpAdapter<'a, S>
where
    S: Service<WarpRequest, Response = WarpResponse, Error = Infallible>,
    S::Future: Send + 'a,
{
    pub fn new(warp_service: S) -> Self {
        Self {
            warp_service,
            _phantom_data: PhantomData,
        }
    }
}

static PLAINTEXT_MIMES: Lazy<HashSet<Mime>> = Lazy::new(|| {
    vec![
        mime::APPLICATION_JAVASCRIPT,
        mime::APPLICATION_JAVASCRIPT_UTF_8,
        mime::APPLICATION_JSON,
    ]
    .into_iter()
    .collect()
});

async fn warp_body_as_lambda_body(
    warp_body: WarpBody,
    parts: &Parts,
) -> Result<LambdaBody, LambdaError> {
    // Concatenate all bytes into a single buffer
    let raw_bytes = warp::hyper::body::to_bytes(warp_body).await?;

    // Attempt to determine the Content-Type
    let content_type: Option<&HeaderValue> = parts.headers.get("Content-Type");
    let content_encoding: Option<&HeaderValue> = parts.headers.get("Content-Encoding");

    // If Content-Encoding is present, assume compression
    // If Content-Type is not present, don't assume is a string
    let body = if let (Some(typ), None) = (content_type, content_encoding) {
        let typ = typ.to_str()?;
        let m = typ.parse::<Mime>()?;
        if PLAINTEXT_MIMES.contains(&m) || m.type_() == mime::TEXT {
            Some(String::from_utf8(raw_bytes.to_vec()).map(LambdaBody::Text)?)
        } else {
            None
        }
    } else {
        None
    };

    // Not a text response, make binary
    Ok(body.unwrap_or_else(|| LambdaBody::Binary(raw_bytes.to_vec())))
}

impl<'a, S> Service<Request> for WarpAdapter<'a, S>
where
    S: Service<WarpRequest, Response = WarpResponse, Error = Infallible> + 'a,
    S::Future: Send + 'a,
{
    type Response = Response<LambdaBody>;
    type Error = LambdaError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send + 'a>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        core::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let query_params = req.query_string_parameters();

        let (mut parts, body) = req.into_parts();
        let body = match body {
            LambdaBody::Empty => WarpBody::empty(),
            LambdaBody::Text(t) => WarpBody::from(t.into_bytes()),
            LambdaBody::Binary(b) => WarpBody::from(b),
        };

        let mut uri = format!("http://{}{}", "127.0.0.1", parts.uri.path());

        if !query_params.is_empty() {
            append_querystring_from_map(&mut uri, query_params.iter());
        }

        parts.uri = warp::hyper::Uri::from_str(uri.as_str())
            .map_err(|e| e)
            .unwrap();
        let warp_request = WarpRequest::from_parts(parts, body);

        // Call warp service with warp request, save future
        let warp_fut = self.warp_service.call(warp_request);

        // Create lambda future
        let fut = async move {
            let warp_response = warp_fut.await?;
            let (parts, res_body): (_, _) = warp_response.into_parts();

            let body = warp_body_as_lambda_body(res_body, &parts).await?;

            let lambda_response = Response::from_parts(parts, body);
            Ok::<Self::Response, Self::Error>(lambda_response)
        };
        Box::pin(fut)
    }
}

fn append_querystring_from_map<'a, I>(uri: &mut String, from_query_params: I)
where
    I: Iterator<Item = (&'a str, &'a str)>,
{
    uri.push('?');
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in from_query_params {
        serializer.append_pair(key, value);
    }
    uri.push_str(serializer.finish().as_str())
}
