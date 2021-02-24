# warp_lambda

A super simple crate to let you use [warp filters](https://github.com/seanmonstar/warp) with [aws lambda runtime](https://github.com/awslabs/aws-lambda-rust-runtime)

> Note: Using fork of [awslabs/aws-lambda-rust-runtime](https://github.com/awslabs/aws-lambda-rust-runtime) by Netlify devs [lamedh-dev/aws-lambda-rust-runtime](https://github.com/lamedh-dev/aws-lambda-rust-runtime)
> Due to issue [#216](https://github.com/awslabs/aws-lambda-rust-runtime/issues/216) and an alternative from Netlify devs [#274](https://github.com/awslabs/aws-lambda-rust-runtime/issues/274)

> `Warning: This is experimental and not production ready! uses non stable version of aws_lambda_rust_runtime`

# Example

Add `warp_lambda`, `warp` and `tokio` to your dependencies:

```toml
tokio = { version = "1.2.0", features = [ "full" ]}
warp = "0.3"
warp_lambda = "0.1"
```

And then get started in your `main.rs`:

```rust
use warp::Filter;

#[tokio::main]
async fn main() {
    // Your warp routes (filters)
    let routes = warp::any().map(|| "Hello, World!");
    // Convert them to a warp service (a tower service implmentation)
    // using `warp::service()`
    let warp_service = warp::service(routes);
    // The warp_lambda::run() function takes care of invoking the aws lambda runtime for you
    warp_lambda::run(warp_service)
        .await
        .expect("An error occured");
}

```

# Deployment

Relevant parts copied over from https://github.com/awslabs/aws-lambda-rust-runtime

#### AWS CLI

To deploy the basic sample as a Lambda function using the [AWS CLI](https://docs.aws.amazon.com/cli/latest/userguide/cli-chap-welcome.html), we first need to manually build it with [`cargo`](https://doc.rust-lang.org/cargo/). Since Lambda uses Amazon Linux, you'll need to target your executable for an `x86_64-unknown-linux-musl` platform.

Run this script once to add the new target:
```bash
$ rustup target add x86_64-unknown-linux-musl
```

Compile one of the examples as a _release_ with a specific _target_ for deployment to AWS:
```bash
$ cargo build --example hello_world --release --target x86_64-unknown-linux-musl
```

For [a custom runtime](https://docs.aws.amazon.com/lambda/latest/dg/runtimes-custom.html), AWS Lambda looks for an executable called `bootstrap` in the deployment package zip. Rename the generated `basic` executable to `bootstrap` and add it to a zip archive.

```bash
$ cp ./target/release/examples/hello ./bootstrap && zip lambda.zip bootstrap && rm bootstrap
```

Now that we have a deployment package (`lambda.zip`), we can use the [AWS CLI](https://aws.amazon.com/cli/) to create a new Lambda function. Make sure to replace the execution role with an existing role in your account!

```bash
$ aws lambda create-function --function-name rustTest \
  --handler doesnt.matter \
  --zip-file fileb://./lambda.zip \
  --runtime provided \
  --role arn:aws:iam::XXXXXXXXXXXXX:role/your_lambda_execution_role \
  --environment Variables={RUST_BACKTRACE=1} \
  --tracing-config Mode=Active
```

**Note:** `--cli-binary-format raw-in-base64-out` is a required
  argument when using the AWS CLI version 2. [More Information](https://docs.aws.amazon.com/cli/latest/userguide/cliv2-migration.html#cliv2-migration-binaryparam)

#### Docker

Alternatively, you can build a Rust-based Lambda function in a [docker mirror of the AWS Lambda provided runtime with the Rust toolchain preinstalled](https://github.com/softprops/lambda-rust).

Running the following command will start a ephemeral docker container which will build your Rust application and produce a zip file containing its binary auto-renamed to `bootstrap` to meet the AWS Lambda's expectations for binaries under `target/lambda/release/{your-binary-name}.zip`, typically this is just the name of your crate if you are using the cargo default binary (i.e. `main.rs`)

```bash
# build and package deploy-ready artifact
$ docker run --rm \
    -v ${PWD}:/code \
    -v ${HOME}/.cargo/registry:/root/.cargo/registry \
    -v ${HOME}/.cargo/git:/root/.cargo/git \
    softprops/lambda-rust
```

## Supported Lambda HTTP Trigger events

* API Gateway (REST API and HTTP API)
* Application Load Balancer

Recommended to use API Gateway  with HTTP API with following parameters.

```
API Type: HTTP
Method: ANY
Resource Path: /{proxy+}
```

## License

MIT
