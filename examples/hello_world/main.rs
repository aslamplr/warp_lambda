use anyhow::{anyhow, Result};
use warp::Filter;

#[tokio::main]
async fn main() -> Result<()> {
    // Your warp routes (filters)
    let routes = warp::any().map(|| "Hello, World!");
    // Convert them to a warp service (a tower service implmentation)
    // using `warp::service()`
    let warp_service = warp::service(routes);
    // The warp_lambda::run() function takes care of invoking the aws lambda runtime for you
    warp_lambda::run(warp_service)
        .await
        .map_err(|err| anyhow!("An error occured `{:#?}`", err))
}
