use core::future::Future;
use std::convert::Infallible;
use std::pin::Pin;

pub use warp;

use lambda_http::{
    handler,
    lambda_runtime::{self, Context},
    Body, Handler, Request, RequestExt, Response,
};
use mime::Mime;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::str::FromStr;
use warp::http::response::Parts;
use warp::http::HeaderValue;
use warp::hyper::Body as WarpBody;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

pub async fn run<Svc>(warp_svc: Svc) -> Result<(), WarpHandlerError>
where
    Svc: tower::Service<WarpRequest, Response = WarpResponse, Error = Infallible> + 'static,
{
    lambda_runtime::run(handler(WarpHandler(warp_svc))).await?;
    Ok(())
}

type WarpRequest = warp::http::Request<warp::hyper::Body>;
type WarpResponse = warp::http::Response<warp::hyper::Body>;

#[derive(thiserror::Error, Debug)]
pub enum WarpHandlerError {
    #[error("This may never occur, it's infallible!!")]
    Infallible(#[from] std::convert::Infallible),
    #[error("Warp error: `{0:#?}`")]
    HyperError(#[from] warp::hyper::Error),
    #[error("Unexpected error: `{0:#?}`")]
    Unexpected(#[from] Error),
}

struct WarpHandler<
    Svc: tower::Service<WarpRequest, Response = WarpResponse, Error = Infallible> + 'static,
>(Svc);

type WarpHandlerFuture<Resp, Err> = Pin<Box<dyn Future<Output = Result<Resp, Err>> + 'static>>;

static PLAINTEXT_MIMES: Lazy<HashSet<Mime>> = Lazy::new(|| {
    vec![
        mime::APPLICATION_JAVASCRIPT,
        mime::APPLICATION_JAVASCRIPT_UTF_8,
        mime::APPLICATION_JSON,
    ]
    .into_iter()
    .collect()
});

async fn warp_body_as_lambda_body(warp_body: WarpBody, parts: &Parts) -> Result<Body, Error> {
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
            Some(String::from_utf8(raw_bytes.to_vec()).map(Body::Text)?)
        } else {
            None
        }
    } else {
        None
    };

    // Not a text response, make binary
    Ok(body.unwrap_or_else(|| Body::Binary(raw_bytes.to_vec())))
}

impl<F> Handler<'_> for WarpHandler<F>
where
    F: tower::Service<WarpRequest, Response = WarpResponse, Error = Infallible> + 'static,
{
    type Error = Error;
    type Response = Response<Body>;
    type Fut = WarpHandlerFuture<Self::Response, Self::Error>;

    #[tracing::instrument(
        name = "warp_lambda::call",
        skip(self, event, context),
        fields(request_id = ?context.request_id)
    )]
    fn call(&mut self, event: Request, context: Context) -> Self::Fut {
        let query_params = event.query_string_parameters();

        let (mut parts, body) = event.into_parts();
        let body = match body {
            Body::Empty => WarpBody::empty(),
            Body::Text(t) => WarpBody::from(t.into_bytes()),
            Body::Binary(b) => WarpBody::from(b),
        };

        let mut uri = format!("http://{}{}", "127.0.0.1", parts.uri.path());

        if !query_params.is_empty() {
            append_querystring_from_map(&mut uri, query_params.iter());
        }

        parts.uri = warp::hyper::Uri::from_str(uri.as_str()).or_else(|e| Err(e)).unwrap();
        let warp_request = WarpRequest::from_parts(parts, body);

        // Call warp service with warp request, save future
        let warp_fut = self.0.call(warp_request);

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
    for (key, value) in from_query_params.into_iter() {
        serializer.append_pair(key, value);
    }
    uri.push_str(serializer.finish().as_str())
}
