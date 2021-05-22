use anyhow::{anyhow, Result};
use tracing_subscriber::fmt::format::FmtSpan;
use warp::Filter;

#[tokio::main]
async fn main() -> Result<()> {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_owned());

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(FmtSpan::CLOSE)
        .json()
        .init();

    // Your warp routes (filters)
    let routes = warp::any()
        .map(|| "Hello, World!")
        .with(warp::log("warp_lambda::tracing::test"))
        .with(warp::trace::request());

    // Convert them to a warp service (a tower service implmentation)
    // using `warp::service()`
    let warp_service = warp::service(routes);
    // The warp_lambda::run() function takes care of invoking the aws lambda runtime for you
    warp_lambda::run(warp_service)
        .await
        .map_err(|err| anyhow!("An error occured `{:#?}`", err))
}
