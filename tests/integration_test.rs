use assert_cmd::prelude::*;
use assert_fs::TempDir;
use predicates::prelude::*;
use std::fs::*;
use std::path::{Path, PathBuf};
use std::process::Command;

use testlib::*;

#[test]
fn creates_a_valid_test_package() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd: Command = Command::cargo_bin("anonymiser")?;
    let input_dir: TempDir = TempDir::new().unwrap();
    create_package(&input_dir, valid_json(), None);
    let output_dir: TempDir = TempDir::new().unwrap();
    cmd.arg("--input")
        .arg(input_dir.path().to_str().unwrap())
        .arg("--output")
        .arg(output_dir.path().to_str().unwrap());
    cmd.assert().success();

    let output_tar_gz: PathBuf = output_dir
        .read_dir()
        .unwrap()
        .last()
        .unwrap()
        .unwrap()
        .path();
    decompress_test_file(&output_tar_gz, &output_dir);
    let metadata_json = get_metadata_json_fields(&output_dir.to_owned());
    assert_eq!(metadata_json.contact_email, "XXXXXXXXX");
    assert_eq!(metadata_json.contact_name, "XXXXXXXXX");
    assert_eq!(
        metadata_json.checksum,
        "fa0c3828d4ad516c5e58d9ddc2739c8cae6701c0000a94e7684d589921787ccd"
    );
    Ok(())
}

#[test]
fn error_if_invalid_tar_file() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd: Command = Command::cargo_bin("anonymiser")?;
    let input_dir: TempDir = TempDir::new().unwrap();
    write(input_dir.join(Path::new("test.tar.gz")), "").unwrap();
    let output_dir: TempDir = TempDir::new().unwrap();
    cmd.arg("--input")
        .arg(input_dir.path().to_str().unwrap())
        .arg("--output")
        .arg(output_dir.path().to_str().unwrap());

    cmd.assert()
        .failure()
        .stdout(predicate::str::contains("failed to iterate over archive"));
    Ok(())
}

#[test]
fn error_if_filename_missing() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd: Command = Command::cargo_bin("anonymiser")?;
    let input_dir: TempDir = TempDir::new().unwrap();
    create_package(&input_dir, json_missing_filename(), None);
    let output_dir: TempDir = TempDir::new().unwrap();
    cmd.arg("--input")
        .arg(input_dir.path().to_str().unwrap())
        .arg("--output")
        .arg(output_dir.path().to_str().unwrap());

    cmd.assert().failure().stdout(predicate::str::contains(
        "Filename is missing from the metadata json",
    ));
    Ok(())
}

#[test]
fn error_if_filename_batch_id_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd: Command = Command::cargo_bin("anonymiser")?;
    let input_dir: TempDir = TempDir::new().unwrap();
    create_package(
        &input_dir,
        valid_json(),
        Some(String::from("INVALID-BATCH")),
    );
    let output_dir: TempDir = TempDir::new().unwrap();
    cmd.arg("--input")
        .arg(input_dir.path().to_str().unwrap())
        .arg("--output")
        .arg(output_dir.path().to_str().unwrap());

    cmd.assert()
        .failure()
        .stdout(predicate::str::contains("No such file or directory"));
    Ok(())
}
