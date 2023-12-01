//! # Test library functions
//!
//! These are common functions used in the integration tests for the script and for the lambda.
use assert_fs::TempDir;
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use serde_json::Value;
use std::fs::*;
use std::path::{Path, PathBuf};
use tar::Archive;
use tar::Builder;

/// # Represents the fields to be anonymised
pub struct MetadataJson {
    pub contact_email: String,
    pub contact_name: String,
    pub checksum: String,
}

/// # Creates a test tar.gz file
pub fn create_package(input_dir: &TempDir, json: &str, file_name: Option<String>) -> PathBuf {
    let package_dir: PathBuf = input_dir.join(PathBuf::from("TDR-2023"));
    let tar_path: PathBuf = input_dir.join(PathBuf::from(
        file_name.unwrap_or(String::from("TDR-2023.tar.gz")),
    ));
    create_dir_all(&package_dir).unwrap();
    let metadata_path: PathBuf = package_dir.join(Path::new("TRE-TDR-2023-metadata.json"));
    let docx_path: PathBuf = package_dir.join(Path::new("test.docx"));

    write(metadata_path, json).unwrap();
    write(docx_path, "").unwrap();
    let tar_gz: File = File::create(tar_path.clone()).unwrap();
    let enc: GzEncoder<File> = GzEncoder::new(tar_gz, Compression::default());
    let mut tar: Builder<GzEncoder<File>> = Builder::new(enc);
    tar.append_dir_all("TDR-2023", &package_dir).unwrap();
    tar_path
}

/// # Decompresses the test tar.gz file
pub fn decompress_test_file(path_to_tar: &PathBuf, output_path: &TempDir) {
    let tar_gz: File = File::open(path_to_tar).unwrap();
    let tar: GzDecoder<File> = GzDecoder::new(tar_gz);
    let mut archive: Archive<GzDecoder<File>> = Archive::new(tar);
    archive.unpack(output_path).unwrap();
}

/// # Read the metadata.json file and parse the fields to be anonymised
pub fn get_metadata_json_fields(output_dir: &Path) -> MetadataJson {
    let metadata: String = read_to_string(
        output_dir
            .join("TST-2023")
            .join("TRE-TST-2023-metadata.json"),
    )
    .unwrap();
    let json_value: Value = serde_json::from_str(&metadata).unwrap();
    metadata_from_json_value(&json_value)
}

/// # Parse the fields from the json value
fn metadata_from_json_value(json_value: &Value) -> MetadataJson {
    let tdr = json_value["parameters"]["TDR"].clone();

    MetadataJson {
        contact_email: tdr["Contact-Email"].as_str().unwrap().to_string(),
        contact_name: tdr["Contact-Name"].as_str().unwrap().to_string(),
        checksum: tdr["Document-Checksum-sha256"]
            .as_str()
            .unwrap()
            .to_string(),
    }
}

/// # An input string with the filename missing
pub fn json_missing_filename() -> &'static str {
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

/// # An valid input string
pub fn valid_json() -> &'static str {
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
