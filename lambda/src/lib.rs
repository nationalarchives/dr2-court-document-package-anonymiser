use anonymise::process_package;
use aws_config::meta::region::RegionProviderChain;
use aws_config::BehaviorVersion;
use aws_lambda_events::sqs::SqsMessage;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use lambda_runtime::Error;
use serde_json::Value;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

struct S3Details {
    bucket: String,
    key: String,
}

fn get_s3_details(record: &SqsMessage) -> Result<S3Details, Error> {
    let body = record
        .body
        .as_ref()
        .ok_or("No body found in the SQS message")?;
    let json: Value = serde_json::from_str(&body)?;
    let parameters = &json["parameters"];
    let bucket = &parameters["s3Bucket"]
        .as_str()
        .ok_or("s3Bucket missing from input")?;
    let key = &parameters["s3Key"]
        .as_str()
        .ok_or("s3Key missing from input")?;
    Ok(S3Details {
        bucket: bucket.to_string(),
        key: key.to_string(),
    })
}

pub async fn process_record(
    record: &SqsMessage,
    working_directory: PathBuf,
    endpoint_url: Option<&str>,
) -> Result<PathBuf, Error> {
    let client = create_client(endpoint_url).await;
    let s3_details = get_s3_details(&record)?;

    let input_file_path = download(
        &client,
        s3_details.bucket,
        s3_details.key,
        &working_directory,
    )
    .await
    .map_err(|_| "Error downloading file from S3")?;
    let output_path = &working_directory.join(PathBuf::from("output"));
    fs::create_dir_all(output_path)?;
    let output_tar_path = process_package(output_path, &input_file_path)?;
    let file_name = output_tar_path
        .file_name()
        .and_then(|oss| oss.to_str())
        .expect("Cannot parse file name from output path");

    let output_bucket = std::env::var("OUTPUT_BUCKET")?;
    upload(&client, &output_tar_path, &output_bucket, file_name)
        .await
        .map_err(|_| "Error uploading file to S3")?;
    return Ok(output_path.clone());
}

async fn upload(
    client: &Client,
    body_path: &PathBuf,
    bucket: &str,
    key: &str,
) -> Result<(), Error> {
    let body = ByteStream::from_path(body_path).await?;
    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(body)
        .send()
        .await?;
    Ok(())
}

async fn download(
    client: &Client,
    bucket: String,
    key: String,
    working_directory: &PathBuf,
) -> Result<PathBuf, Error> {
    let destination = working_directory.join(PathBuf::from(&key));
    let mut file = File::create(&destination)?;

    let mut object = client.get_object().bucket(bucket).key(&key).send().await?;

    while let Some(bytes) = object.body.try_next().await? {
        file.write_all(&bytes)?;
    }

    Ok(destination)
}

async fn create_client(potential_endpoint_url: Option<&str>) -> Client {
    let endpoint_url = potential_endpoint_url.unwrap_or("https://s3.eu-west-2.amazonaws.com");
    let region_provider = RegionProviderChain::default_provider().or_else("eu-west-2");

    let config = aws_config::defaults(BehaviorVersion::latest())
        .region(region_provider)
        .endpoint_url(endpoint_url)
        .load()
        .await;
    Client::new(&config)
}

#[cfg(test)]
mod test {
    use crate::{create_client, get_s3_details};
    use aws_lambda_events::sqs::SqsMessage;

    #[tokio::test]
    async fn test_create_client_with_default_region() {
        let client = create_client(None).await;
        let config = client.config();

        assert_eq!(config.region().unwrap().to_string(), "eu-west-2");
    }

    #[tokio::test]
    async fn test_get_s3_details() {
        let message = SqsMessage {
            body: Option::from(
                "{\"parameters\": {\"s3Bucket\": \"testBucket\", \"s3Key\": \"testKey\"}}"
                    .to_owned(),
            ),
            ..Default::default()
        };
        let details = get_s3_details(&message).unwrap();
        assert_eq!(details.bucket, "testBucket");
        assert_eq!(details.key, "testKey");
    }
}
