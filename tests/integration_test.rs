use std::fs::*;
use std::path::{Path, PathBuf};
use std::process::Command;
use predicates::prelude::*;
use flate2::{
    read::GzDecoder,
    Compression,
    write::GzEncoder
};
use tar::Archive;
use assert_cmd::prelude::*;
use assert_fs::TempDir;
use serde_json::Value;

fn missing_filename() -> &'static str {
    r#"
    {
      "parameters": {
        "PARSER": {
          "name": "test"
        },
        "TDR": {
          "Document-Checksum-sha256": "3c7b9ef49d36659762c34c63bae05b4cf07d6406c2736720385ed0c6f015840a"
        }
      }
    }
    "#
}

fn valid_json() -> &'static str {
    r#"
    {
      "parameters": {
        "PARSER": {
          "name": "test"
        },
        "TDR": {
          "Document-Checksum-sha256": "3c7b9ef49d36659762c34c63bae05b4cf07d6406c2736720385ed0c6f015840a"
        },
        "TRE": {
          "payload": {
            "filename": "test.docx"
          }
        }
      }
    }
    "#
}
fn create_package(input_dir: &TempDir, json: &str, file_name: Option<String>) {
    let package_dir = input_dir.join(PathBuf::from("TDR-2023"));
    let tar_path = input_dir.join(PathBuf::from(file_name.unwrap_or(String::from("TDR-2023.tar.gz"))));
    create_dir_all(&package_dir).unwrap();
    let metadata_path = package_dir.join(Path::new("TRE-TDR-2023-metadata.json"));
    let docx_path = package_dir.join(Path::new("test.docx"));

    write(metadata_path, json).unwrap();
    write(docx_path, "").unwrap();
    let tar_gz = File::create(tar_path).unwrap();
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);
    tar.append_dir_all("TDR-2023", &package_dir).unwrap();
}

fn decompress_test_file(path_to_tar: &PathBuf, output_path: &TempDir) {
    let tar_gz = File::open(path_to_tar).unwrap();
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    archive.unpack(&output_path).unwrap();
}

#[test]
fn creates_a_valid_test_package() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("anonymiser")?;
    let input_dir = TempDir::new().unwrap();
    create_package(&input_dir, valid_json(), None);
    let output_dir = TempDir::new().unwrap();
    cmd
        .arg("--input")
        .arg(input_dir.path().to_str().unwrap())
        .arg("--output")
        .arg(output_dir.path().to_str().unwrap());
    cmd.assert().success();

    let output_tar_gz = output_dir.read_dir().unwrap().last().unwrap().unwrap().path();
    decompress_test_file(&output_tar_gz, &output_dir);
    let metadata = read_to_string(&output_dir.join("TST-2023").join("TRE-TST-2023-metadata.json")).unwrap();
    let json_value: Value = serde_json::from_str(&metadata).unwrap();


    assert_eq!(json_value["parameters"]["TDR"]["Contact-Email"], "XXXXXXXXX");
    assert_eq!(json_value["parameters"]["TDR"]["Contact-Name"], "XXXXXXXXX");
    assert_eq!(json_value["parameters"]["TDR"]["Document-Checksum-sha256"], "f836c06d224fdd204dc6152ea3420ad68c667880962ae10af7acefb9f32588b0");
    Ok(())
}

#[test]
fn error_if_invalid_tar_file() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("anonymiser")?;
    let input_dir = TempDir::new().unwrap();
    write(input_dir.join(Path::new("test.tar.gz")), "").unwrap();
    let output_dir = TempDir::new().unwrap();
    cmd
        .arg("--input")
        .arg(input_dir.path().to_str().unwrap())
        .arg("--output")
        .arg(output_dir.path().to_str().unwrap());

    cmd.assert().failure()
        .stdout(predicate::str::contains("failed to iterate over archive"));
    Ok(())
}

#[test]
fn error_if_filename_missing() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("anonymiser")?;
    let input_dir = TempDir::new().unwrap();
    create_package(&input_dir, missing_filename(), None);
    let output_dir = TempDir::new().unwrap();
    cmd
        .arg("--input")
        .arg(input_dir.path().to_str().unwrap())
        .arg("--output")
        .arg(output_dir.path().to_str().unwrap());

    cmd.assert().failure()
        .stdout(predicate::str::contains("Filename is missing from the metadata json"));
    Ok(())
}

#[test]
fn error_if_filename_batch_id_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("anonymiser")?;
    let input_dir = TempDir::new().unwrap();
    create_package(&input_dir, valid_json(), Some(String::from("INVALID-BATCH")));
    let output_dir = TempDir::new().unwrap();
    cmd
        .arg("--input")
        .arg(input_dir.path().to_str().unwrap())
        .arg("--output")
        .arg(output_dir.path().to_str().unwrap());

    cmd.assert().failure()
        .stdout(predicate::str::contains("No such file or directory"));
    Ok(())
}
