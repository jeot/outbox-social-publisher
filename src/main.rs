use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    publo::run().await
}
