use core::future::Future;
use std::convert::Infallible;
use std::pin::Pin;

pub use netlify_lambda_http as lambda_http;
pub use warp;

use aws_lambda_events::encodings::Body as LambdaBody;
use netlify_lambda_http::{
    handler,
    lambda::{self, Context},
    Handler, Request, Response,
};
use warp::hyper::Body as WarpBody;

type Error = Box<dyn std::error::Error + Send + Sync + 'static>;

pub async fn run<Svc>(warp_svc: Svc) -> Result<(), WarpHandlerError>
where
    Svc: tower::Service<WarpRequest, Response = WarpResponse, Error = Infallible> + 'static,
{
    lambda::run(handler(WarpHandler(warp_svc))).await?;
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

impl<F> Handler for WarpHandler<F>
where
    F: tower::Service<WarpRequest, Response = WarpResponse, Error = Infallible> + 'static,
{
    type Response = Response<LambdaBody>;
    type Error = Error;
    type Fut = WarpHandlerFuture<Self::Response, Self::Error>;

    fn call(&mut self, event: Request, _context: Context) -> Self::Fut {
        let (parts, in_body) = event.into_parts();
        let body = match in_body {
            LambdaBody::Binary(data) => WarpBody::from(data),
            LambdaBody::Text(text) => WarpBody::from(text),
            LambdaBody::Empty => WarpBody::empty(),
        };
        let warp_request = WarpRequest::from_parts(parts, body);
        let warp_fut = self.0.call(warp_request);
        let fut = async {
            let warp_response = warp_fut.await?;
            let (parts, res_body) = warp_response.into_parts();
            let raw_bytes = warp::hyper::body::to_bytes(res_body).await?;
            let body = LambdaBody::from(raw_bytes.to_vec());
            let lambda_response = Response::from_parts(parts, body);
            Ok::<Self::Response, Self::Error>(lambda_response)
        };
        Box::pin(fut)
    }
}
