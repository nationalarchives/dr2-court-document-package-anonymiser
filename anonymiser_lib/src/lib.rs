//! ## Court document package anonymiser library
//!
//! This library contains common code shared between the anonymiser script and the lambda.
use clap::Parser;
use docx_rs::*;
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use serde_json::{json, Value};
use sha256::try_digest;

use std::fs::{remove_file, DirEntry};
use std::io::ErrorKind;
use std::{fs, fs::File, io, io::Error, io::Read, path::Path, path::PathBuf};
use tar::{Archive, Builder};

/// # A struct representing the input arguments
#[derive(Parser)]
#[clap(name = "anonymiser")]
pub struct Opt {
    /// Input folder
    #[clap(long, short, value_parser)]
    pub input: String,

    /// Output folder
    #[clap(long, short, value_parser)]
    pub output: String,
}

/// # Package processor
/// This takes an output directory path and a path to a tar.gz file as input and anonymises them with the following steps:
///
/// * It replaces the values of Contact-Email and Contact-Name with XXXXXXX
/// * It generates a new docx file which only contains the name of the judgment.
/// * It updates the checksum field with the calculated checksum of the new docx file.
/// * It renames the folder and metadata file from TDR-xxx to TST-xxx.
/// * It creates a new tar.gz folder in the output directory.
/// * It deletes the uncompressed folder in the output directory.
pub fn process_package(dir_output: &PathBuf, file: &PathBuf) -> Result<PathBuf, Error> {
    let tar_gz_file_name: String = file
        .file_name()
        .and_then(|name| name.to_os_string().into_string().ok())
        .ok_or("Error getting the file name from the file")
        .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;

    let output_tar_gz_path: PathBuf =
        Path::new(&dir_output).join(Path::new(&tar_gz_file_name.replace("TDR", "TST")));
    let uncompressed_folder_input_path: &PathBuf = &file.with_extension("").with_extension("");
    let input_batch_reference: String = uncompressed_folder_input_path
        .file_name()
        .and_then(|name| name.to_str().map(|name| name.replace("TRE-", "")))
        .ok_or(Error::new(
            ErrorKind::InvalidInput,
            "Cannot get a batch reference from the file name",
        ))?;
    let output_batch_reference: &String = &input_batch_reference.replace("TDR", "TST");

    let extracted_output_original_name: PathBuf =
        dir_output.join(PathBuf::from(&input_batch_reference));
    let extracted_output_path: PathBuf = dir_output.join(PathBuf::from(output_batch_reference));

    let output_path_with_file = |file_name: &str| -> PathBuf {
        let output_path = extracted_output_path.clone();
        output_path.join(PathBuf::from(file_name))
    };

    fs::create_dir_all(extracted_output_path.clone())?;

    decompress_file(file, dir_output)?;

    let metadata_input_file_path: PathBuf =
        output_path_with_file(format!("TRE-{input_batch_reference}-metadata.json").as_str());
    let metadata_output_file_path: PathBuf =
        output_path_with_file(format!("TRE-{output_batch_reference}-metadata.json").as_str());

    if extracted_output_path.exists() {
        fs::remove_dir_all(&extracted_output_path)?;
    }
    fs::rename(extracted_output_original_name, &extracted_output_path)?;
    fs::rename(metadata_input_file_path, &metadata_output_file_path)?;

    let mut metadata_json_value: Value = parse_metadata_json(&metadata_output_file_path)?;

    let docx_checksum =
        create_docx_with_checksum(&extracted_output_path, &mut metadata_json_value)?;

    update_json_file(
        &metadata_output_file_path,
        docx_checksum,
        &mut metadata_json_value,
    )?;

    if_present_delete(output_path_with_file(
        format!("{input_batch_reference}.xml").as_str(),
    ))?;
    if_present_delete(output_path_with_file("parser.log"))?;

    tar_folder(
        &output_tar_gz_path,
        &extracted_output_path,
        output_batch_reference,
    )?;

    fs::remove_dir_all(&extracted_output_path)?;
    Ok(output_tar_gz_path)
}

/// # Creates a docx and returns a checksum
///
/// This creates a new docx file with the name parsed from the metadata filename.
///
/// It then writes the judgment name to the docx file and saves it to the output directory.
/// If there is no judgment name, it uses the filename
///
/// Finally, it generates a sha256 checksum for the new file and returns it.
fn create_docx_with_checksum(
    extracted_output_path: &Path,
    metadata_json_value: &mut Value,
) -> Result<String, Error> {
    let docx_file_name: &str = metadata_json_value["parameters"]["TRE"]["payload"]["filename"]
        .as_str()
        .ok_or("'filename' is missing from the metadata json")
        .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;

    let judgment_name: &str = metadata_json_value["parameters"]["PARSER"]["name"]
        .as_str()
        .unwrap_or(docx_file_name);
    let docx_path: PathBuf = extracted_output_path.join(PathBuf::from(docx_file_name));

    let file: File = File::create(&docx_path)?;
    Docx::new()
        .add_paragraph(Paragraph::new().add_run(Run::new().add_text(judgment_name)))
        .build()
        .pack(file)?;

    let docx_checksum: String = try_digest(&docx_path).unwrap();
    Ok(docx_checksum)
}

/// # Helper function to delete a file if present
fn if_present_delete(path: PathBuf) -> io::Result<()> {
    if path.exists() {
        remove_file(path)?
    }
    Ok(())
}

/// # Helper function to check if a file does not start with `.`
fn is_not_hidden(entry: &DirEntry) -> bool {
    entry
        .file_name()
        .to_str()
        .map(|file_name| !file_name.starts_with('.'))
        .unwrap_or(false)
}

/// # Helper function to check if the entry is a file
fn is_file(entry: &DirEntry) -> bool {
    !entry.path().is_dir()
}

/// # List files in input directory
///
/// This takes a directory path and returns a list of paths of all files on that level.
/// It will not recursively search subdirectories.
pub fn files_in_input_dir(directory_path: &PathBuf) -> Result<Vec<PathBuf>, Error> {
    let path_list: Vec<PathBuf> = fs::read_dir(directory_path)
        .unwrap()
        .filter_map(|e| {
            let entry: DirEntry = e.ok()?;
            if is_file(&entry) && is_not_hidden(&entry) {
                Some(entry.path())
            } else {
                None
            }
        })
        .collect::<Vec<PathBuf>>();
    Ok(path_list)
}

/// # Tars and Gzips the specified folder
///
/// This creates a tar file at `tar_path`, compresses everything in `path_to_compress` and names it with `folder_name`
fn tar_folder(
    tar_path: &PathBuf,
    path_to_compress: &PathBuf,
    folder_name: &String,
) -> Result<(), Error> {
    let tar_gz: File = File::create(tar_path)?;
    let enc: GzEncoder<File> = GzEncoder::new(tar_gz, Compression::default());
    let mut tar: Builder<GzEncoder<File>> = Builder::new(enc);
    tar.append_dir_all(folder_name, path_to_compress)?;
    Ok(())
}

/// # Anonymise the contact fields and update the checksum
fn update_json_file(
    metadata_file_name: &PathBuf,
    checksum: String,
    json_value: &mut Value,
) -> Result<(), Error> {
    let tdr: &mut Value = &mut json_value["parameters"]["TDR"];
    tdr["Contact-Email"] = json!("XXXXXXXXX");
    tdr["Contact-Name"] = json!("XXXXXXXXX");
    tdr["Document-Checksum-sha256"] = json!(checksum);
    fs::write(metadata_file_name, json_value.to_string())
}

/// # Untar and unzip the input tar.gz file
fn decompress_file(path_to_tar: &PathBuf, output_path: &PathBuf) -> Result<(), Error> {
    let tar_gz: File = File::open(path_to_tar)?;
    let tar: GzDecoder<File> = GzDecoder::new(tar_gz);
    let mut archive: Archive<GzDecoder<File>> = Archive::new(tar);
    archive.unpack(output_path)?;
    Ok(())
}

/// # Read the metadata.json file and parse it into a serde `Value`
fn parse_metadata_json(metadata_file_path: &PathBuf) -> Result<Value, Error> {
    let mut metadata_file: File = File::open(metadata_file_path)?;
    let mut metadata_json_as_string: String = String::new();
    metadata_file.read_to_string(&mut metadata_json_as_string)?;
    Ok(serde_json::from_str(&metadata_json_as_string)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::create_docx_with_checksum;
    use assert_fs::TempDir;
    use std::fs::{read_dir, read_to_string};
    use testlib::create_package;

    #[test]
    fn test_create_docx_with_checksum() {
        let output_path = TempDir::new().unwrap();
        let mut json_value = json!({
            "parameters": {
                "PARSER": {
                    "name" : "test-name"
                },
                "TRE": {
                    "payload": {
                        "filename": "test-file-name.docx"
                    }
                }
            }
        });
        let docx_checksum =
            create_docx_with_checksum(&output_path.to_owned(), &mut json_value).unwrap();
        let output_files = read_dir(&output_path.to_owned()).unwrap();
        let filename = &output_files.last().unwrap().unwrap().file_name();

        assert_eq!(
            filename.to_str().unwrap().to_string(),
            "test-file-name.docx"
        );
        assert_eq!(
            docx_checksum,
            "a951e0d7f11d9d2fa8c9508ee4b25944bb5810364089fc33221b1ec038eefd37"
        )
    }

    #[test]
    fn test_create_docx_with_checksum_missing_metadata_filename() {
        let output_path = TempDir::new().unwrap();
        let mut json_value = json!({
            "parameters": {
                "PARSER": {
                    "name" : "test-name"
                }
            }
        });
        let err = create_docx_with_checksum(&output_path.to_owned(), &mut json_value).unwrap_err();
        assert_eq!(
            err.to_string(),
            "'filename' is missing from the metadata json"
        )
    }

    #[test]
    fn test_parse_metadata_json_parses_data_into_value() {
        let output_dir = TempDir::new().unwrap();
        let metadata_path = &output_dir.join(PathBuf::from("metadata.json"));
        fs::write(&metadata_path, r#"{"a": "b"}"#.as_bytes()).unwrap();
        let json = parse_metadata_json(&metadata_path).unwrap();
        assert_eq!(&json["a"], "b")
    }

    #[test]
    fn test_decompress_file() {
        let input_dir = TempDir::new().unwrap();
        let output_dir = TempDir::new().unwrap();
        let tar_path = create_package(&input_dir, "{}", None);
        decompress_file(&tar_path, &output_dir.to_owned()).unwrap();
        assert!(output_dir
            .join(PathBuf::from("TDR-2023/test.docx"))
            .exists());
        assert!(output_dir
            .join(PathBuf::from("TDR-2023/TRE-TDR-2023-metadata.json"))
            .exists());
    }

    #[test]
    fn test_update_json_file() {
        let output_dir = TempDir::new().unwrap();
        let metadata_path = &output_dir.join(PathBuf::from("metadata.json"));
        let mut json_value = json!({
            "parameters": {
                "TDR": {
                    "Contact-Email" : "test-email",
                    "Contact-Email2": "test-email-2",
                    "TDR-Contact-Name": "tdr-contact-name",
                    "Contact-Name" : "test-name",
                    "Document-Checksum-sha256": "test-checksum"
                }
            }
        });
        update_json_file(&metadata_path, "abcde".to_owned(), &mut json_value).unwrap();
        let metadata_json_string = read_to_string(&metadata_path).unwrap();
        let expected_json = r#"{"parameters":{"TDR":{"Contact-Email":"XXXXXXXXX","Contact-Email2":"test-email-2","Contact-Name":"XXXXXXXXX","Document-Checksum-sha256":"abcde","TDR-Contact-Name":"tdr-contact-name"}}}"#;
        assert_eq!(metadata_json_string, expected_json);
    }

    #[test]
    fn test_tar_folder_creates_a_new_tar() {
        let tar_dir = TempDir::new().unwrap();
        let output_dir = TempDir::new().unwrap();
        let folder_name: String = String::from("test_name");
        let tar_file_path = tar_dir.join("test.tar.gz");
        tar_folder(&tar_file_path, &output_dir.to_owned(), &folder_name).unwrap();

        assert!(tar_file_path.exists());
    }
}
