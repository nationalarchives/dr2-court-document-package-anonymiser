use aws_lambda_events::event::sqs::SqsEvent;
use lambda;
use lambda_runtime::{run, service_fn, Error, LambdaEvent};
use std::path::PathBuf;
use tokio;

async fn function_handler(event: LambdaEvent<SqsEvent>) -> Result<(), Error> {
    for record in event.payload.records.iter() {
        lambda::process_record(record, PathBuf::from("/tmp"), None).await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .with_target(false)
        .without_time()
        .init();

    run(service_fn(function_handler)).await
}
