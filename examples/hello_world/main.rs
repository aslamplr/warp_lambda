use anyhow::{anyhow, Result};
use warp::Filter;

#[tokio::main]
async fn main() -> Result<()> {
    let routes = warp::any().map(|| "Hello, World!");
    let warp_service = warp::service(routes);
    warp_lambda::run(warp_service)
        .await
        .map_err(|err| anyhow!("An error occured `{:#?}`", err))
}
