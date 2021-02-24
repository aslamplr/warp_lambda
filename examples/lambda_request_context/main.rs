use anyhow::{anyhow, Result};
use warp::Filter;

use warp_lambda::lambda_http::request::RequestContext;

#[tokio::main]
async fn main() -> Result<()> {
    // Your warp routes (filters)
    let a_route = warp::path!("hello" / String)
        .and(warp::get())
        .and(warp::path::end())
        .and(warp::filters::ext::get::<RequestContext>())
        .map(|name, aws_req_ctx| {
            // Request context is useful for extracting the request-id, or
            // Authorizer configured with API Gateway such as cognito related jwt
            // Or custom authorizer etc.
            let context = match aws_req_ctx {
                // Request context when invoked from an ALB event
                RequestContext::Alb(alb) => format!("::ALB:: {:?}", alb),
                // Request context when invoked from an API Gateway REST event
                RequestContext::ApiGatewayV1(api_gw) => format!("::API_GW:: {:?}", api_gw),
                // Request context when invoked from an API Gateway HTTP event
                RequestContext::ApiGatewayV2(api_gw2) => format!("::API_GW(V2):: {:?}", api_gw2),
            };
            format!("Hello {}! request context for debug {}", name, context)
        });
    // Convert them to a warp service (a tower service implmentation)
    // using `warp::service()`
    let warp_service = warp::service(a_route);
    // The warp_lambda::run() function takes care of invoking the aws lambda runtime for you
    warp_lambda::run(warp_service)
        .await
        .map_err(|err| anyhow!("An error occured `{:#?}`", err))
}
