use assert_fs::TempDir;
use aws_lambda_events::sqs::SqsMessage;
use lambda::process_record;
use std::env::set_var;
use std::fs::{read, write};
use std::path::PathBuf;
use testlib::*;
use tokio;
use wiremock::http::Method;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn downloads_the_live_package_uploads_anonymised_package() {
    let input_dir: TempDir = TempDir::new().unwrap();
    let tar_path = create_package(&input_dir, valid_json(), None);
    let test_input_bucket = "test-input-bucket";
    let test_output_bucket = "test-output-bucket";
    set_var("OUTPUT_BUCKET", test_output_bucket);

    let test_download_key = tar_path
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .unwrap();
    let test_upload_key = test_download_key.replace("TDR", "TST");
    let test_string = format!(
        "{{\"parameters\": {{\"s3Bucket\": \"{}\", \"s3Key\": \"{}\"}}}}",
        test_input_bucket, test_download_key
    );
    let message = SqsMessage {
        body: Option::from(test_string),
        ..Default::default()
    };

    let get_object_path = format!("/{}/{}", test_input_bucket, test_download_key);
    let put_object_path = format!("/{}/{}", test_output_bucket, test_upload_key);
    let mock_server = MockServer::start().await;
    let bytes = read(tar_path).unwrap();

    Mock::given(method("GET"))
        .and(path(get_object_path))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
        .mount(&mock_server)
        .await;
    Mock::given(method("PUT"))
        .and(path(put_object_path))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;
    let uri = mock_server.uri();
    let _ = process_record(&message, input_dir.to_owned(), Option::from(uri.as_str()))
        .await
        .unwrap();

    let requests = &mock_server.received_requests().await.unwrap();
    let put_request = requests
        .iter()
        .filter(|req| req.method == Method::Put)
        .last()
        .unwrap();

    let path_to_output_file = input_dir.to_owned().join("output.tar.gz");
    let output_dir = TempDir::new().unwrap();
    write(&path_to_output_file, &put_request.body).unwrap();
    decompress_test_file(&path_to_output_file, &output_dir);
    let metadata_json = get_metadata_json_fields(&output_dir.to_owned());
    assert_eq!(metadata_json.contact_email, "XXXXXXXXX");
    assert_eq!(metadata_json.contact_name, "XXXXXXXXX");
    assert_eq!(
        metadata_json.checksum,
        "fa0c3828d4ad516c5e58d9ddc2739c8cae6701c0000a94e7684d589921787ccd"
    );
}

#[tokio::test]
async fn error_if_key_is_missing_from_bucket() {
    let test_input_bucket = "test-input-bucket";
    let test_output_bucket = "test-output-bucket";
    set_var("OUTPUT_BUCKET", test_output_bucket);

    let test_download_key = "missing-key.tar.gz";
    let get_object_path = format!("/{}/{}", test_input_bucket, test_download_key);
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path(get_object_path))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;
    let test_string = format!(
        "{{\"parameters\": {{\"s3Bucket\": \"{}\", \"s3Key\": \"{}\"}}}}",
        test_input_bucket, test_download_key
    );
    let message = SqsMessage {
        body: Option::from(test_string),
        ..Default::default()
    };
    let err = process_record(
        &message,
        PathBuf::from("/tmp"),
        Option::from(mock_server.uri().as_str()),
    )
    .await
    .unwrap_err();
    assert_eq!(err.to_string(), "Error downloading file from S3")
}

#[tokio::test]
async fn error_if_key_is_not_a_tar_file() {
    let test_input_bucket = "test-input-bucket";
    let test_output_bucket = "test-output-bucket";
    set_var("OUTPUT_BUCKET", test_output_bucket);

    let test_download_key = "test.tar.gz";
    let get_object_path = format!("/{}/{}", test_input_bucket, test_download_key);
    let mock_server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(get_object_path))
        .respond_with(ResponseTemplate::new(200).set_body_bytes("test".as_bytes()))
        .mount(&mock_server)
        .await;
    let test_string = format!(
        "{{\"parameters\": {{\"s3Bucket\": \"{}\", \"s3Key\": \"{}\"}}}}",
        test_input_bucket, test_download_key
    );
    let message = SqsMessage {
        body: Option::from(test_string),
        ..Default::default()
    };
    let err = process_record(
        &message,
        PathBuf::from("/tmp"),
        Option::from(mock_server.uri().as_str()),
    )
    .await
    .unwrap_err();
    assert_eq!(err.to_string(), "failed to iterate over archive")
}

#[tokio::test]
async fn error_if_upload_fails() {
    let input_dir: TempDir = TempDir::new().unwrap();
    let tar_path = create_package(&input_dir, valid_json(), None);
    let test_input_bucket = "test-input-bucket";
    let test_output_bucket = "test-output-bucket";
    set_var("OUTPUT_BUCKET", test_output_bucket);

    let test_download_key = tar_path
        .file_name()
        .and_then(|file_name| file_name.to_str())
        .unwrap();
    let test_upload_key = test_download_key.replace("TDR", "TST");
    let test_string = format!(
        "{{\"parameters\": {{\"s3Bucket\": \"{}\", \"s3Key\": \"{}\"}}}}",
        test_input_bucket, test_download_key
    );
    let message = SqsMessage {
        body: Option::from(test_string),
        ..Default::default()
    };

    let get_object_path = format!("/{}/{}", test_input_bucket, test_download_key);
    let put_object_path = format!("/{}/{}", test_input_bucket, test_upload_key);
    let mock_server = MockServer::start().await;
    let bytes = read(tar_path).unwrap();

    Mock::given(method("GET"))
        .and(path(get_object_path))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(bytes))
        .mount(&mock_server)
        .await;
    Mock::given(method("PUT"))
        .and(path(put_object_path))
        .respond_with(ResponseTemplate::new(401))
        .mount(&mock_server)
        .await;
    let err = process_record(
        &message,
        input_dir.to_owned(),
        Option::from(mock_server.uri().as_str()),
    )
    .await
    .unwrap_err();
    assert_eq!(err.to_string(), "Error uploading file to S3")
}

#[tokio::test]
async fn error_for_invalid_input() {
    let test_string_missing_bucket = "{\"parameters\": {\"s3Key\": \"key\"}}";
    let test_string_missing_key = "{\"parameters\": {\"s3Bucket\": \"bucket\"}}";
    let missing_body_message = SqsMessage::default();
    let missing_bucket_message = SqsMessage {
        body: Option::from(test_string_missing_bucket.to_string()),
        ..Default::default()
    };
    let missing_key_message = SqsMessage {
        body: Option::from(test_string_missing_key.to_string()),
        ..Default::default()
    };
    let uri = Option::from("http://test.com");
    let missing_body_err = process_record(
        &missing_body_message,
        TempDir::new().unwrap().to_owned(),
        uri,
    )
    .await
    .unwrap_err();
    let missing_bucket_err = process_record(
        &missing_bucket_message,
        TempDir::new().unwrap().to_owned(),
        uri,
    )
    .await
    .unwrap_err();
    let missing_key_err = process_record(
        &missing_key_message,
        TempDir::new().unwrap().to_owned(),
        uri,
    )
    .await
    .unwrap_err();

    assert_eq!(
        missing_body_err.to_string(),
        "No body found in the SQS message"
    );
    assert_eq!(
        missing_bucket_err.to_string(),
        "s3Bucket missing from input"
    );
    assert_eq!(missing_key_err.to_string(), "s3Key missing from input");
}
