use std::{
    fs,
    fs::File,
    path::PathBuf,
    io::Read,
    io::Error,
    path::Path,
    process::exit
};
use std::fs::DirEntry;
use std::io::ErrorKind;
use sha256::{try_digest};
use flate2::{
    read::GzDecoder,
    Compression,
    write::GzEncoder
};
use tar::Archive;
use simple_logger::SimpleLogger;
use log::{
    self,
    LevelFilter
};
use serde_json::{json, Value};
use docx_rs::*;
use clap::Parser;

#[derive(Parser)]
#[clap(name = "anonymiser")]
struct Opt {
    /// Input folder
    #[clap(long, short, value_parser)]
    input: String,

    /// Output folder
    #[clap(long, short, value_parser)]
    output: String,
}
fn main() {
    SimpleLogger::new().with_level(LevelFilter::Info).init().unwrap();
    let opt = Opt::parse();
    let dir_input = PathBuf::from(shellexpand::full(&opt.input).unwrap().to_string());
    let dir_output = PathBuf::from(shellexpand::full(&opt.output).unwrap().to_string());
    let files = list_files_in_input_dir(&dir_input).unwrap();
    for file in files {
        match process_package(&dir_output, &file) {
            Ok(_) => {
                log::info!("Processed {}", file.file_name().and_then(|name| name.to_str()).unwrap() )
            },
            Err(err) =>  {
                log::error!("Error: {:?}", err);
                exit(1);
            }
        };
    }
}

fn process_package(dir_output: &PathBuf, file: &PathBuf) -> Result<(), Error> {
    let tar_gz_file_name = file.file_name()
        .and_then(|name| name.to_os_string().into_string().ok())
        .ok_or("Error getting the file name from the file")
        .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;


    let output_tar_gz_path: PathBuf = Path::new(&dir_output).join(Path::new(&tar_gz_file_name.replace("TDR", "TST")));
    let uncompressed_folder_input_path = &file.with_extension("").with_extension("");
    let input_batch_reference = uncompressed_folder_input_path.file_name().and_then(|name| name.to_str())
        .ok_or(Error::new(ErrorKind::InvalidInput, "Cannot get a batch reference from the file name"))?;
    let output_batch_reference = &input_batch_reference.replace("TDR", "TST");

    let extracted_output_original_name = dir_output.join(PathBuf::from(&input_batch_reference));
    let extracted_output_path = dir_output.join(PathBuf::from(output_batch_reference));

    fs::create_dir_all(extracted_output_path.clone())?;

    decompress_file(&file, &dir_output)?;

    let metadata_input_file_path = &extracted_output_path.join(PathBuf::from(format!("TRE-{}-metadata.json", &input_batch_reference)));
    let metadata_output_file_path = &extracted_output_path.join(PathBuf::from(format!("TRE-{}-metadata.json", &output_batch_reference)));

    if extracted_output_path.exists() {
        fs::remove_dir_all(&extracted_output_path)?;
    }
    fs::rename(&extracted_output_original_name, &extracted_output_path)?;
    fs::rename(&metadata_input_file_path, &metadata_output_file_path)?;

    let mut json_value = parse_json(&metadata_output_file_path)?;

    let docx_file_name = json_value["parameters"]["TRE"]["payload"]["filename"].as_str()
        .ok_or("Filename is missing from the metadata json")
        .map_err(|e| Error::new(ErrorKind::InvalidInput, e))?;

    let judgment_name = json_value["parameters"]["PARSER"]["name"].to_string();
    let docx_path_string = extracted_output_path.join(PathBuf::from(docx_file_name));

    create_docx(&docx_path_string, judgment_name)?;

    let checksum = checksum(&docx_path_string);

    update_json_file(&metadata_output_file_path, checksum, &mut json_value)?;

    _ = fs::remove_file(extracted_output_path.join(PathBuf::from(format!("{}.xml", input_batch_reference).as_str())));
    _ = fs::remove_file(extracted_output_path.join(PathBuf::from("parser.log")));

    tar_folder(&output_tar_gz_path, &extracted_output_path, &output_batch_reference)?;

    fs::remove_dir_all(&extracted_output_path)
}

fn is_hidden(entry: &DirEntry) -> bool {
    entry.file_name()
        .to_str()
        .map(|s| s.starts_with("."))
        .unwrap_or(false)
}

fn is_directory(entry: &DirEntry) -> bool {
    entry.path().is_dir()
}
fn list_files_in_input_dir(directory_path: &PathBuf) -> Result<Vec<PathBuf>, Error> {
    let path_list = fs::read_dir(directory_path).unwrap()
        .filter_map(|e| {
            let entry = e.ok()?;
            if !is_hidden(&entry) && !is_directory(&entry) { Some(entry.path()) } else { None }
        })
        .collect::<Vec<PathBuf>>();
    Ok(path_list)
}

fn checksum(path_string: &PathBuf) -> String {
    try_digest(path_string).unwrap()
}

fn tar_folder(tar_path: &PathBuf, path_to_compress: &PathBuf, folder_name: &String) -> Result<(), Error> {
    let tar_gz = File::create(tar_path)?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);
    tar.append_dir_all(folder_name, &path_to_compress)?;
    Ok(())
}

fn create_docx(docx_path: &PathBuf, judgment_name: String) -> Result<(), Error> {
    let file = File::create(&docx_path)?;
    Docx::new()
        .add_paragraph(Paragraph::new().add_run(Run::new().add_text(judgment_name)))
        .build()
        .pack(file)?;
    Ok(())
}

fn update_json_file(metadata_file_name: &PathBuf, checksum: String, json_value: &mut Value) -> Result<(), Error> {
    json_value["parameters"]["TDR"]["Contact-Email"] = json!("XXXXXXXXX");
    json_value["parameters"]["TDR"]["Contact-Name"] = json!("XXXXXXXXX");
    json_value["parameters"]["TDR"]["Document-Checksum-sha256"] = json!(checksum);
    fs::write(metadata_file_name, json_value.to_string())
}

fn decompress_file(path_to_tar: &PathBuf, output_path: &PathBuf) -> Result<(), Error> {
    let tar_gz = File::open(path_to_tar)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    archive.unpack(&output_path)?;
    Ok(())
}
fn parse_json(metadata_file_path: &PathBuf) -> Result<Value, Error> {
    let mut file = File::open(metadata_file_path)?;
    let mut data = String::new();
    file.read_to_string(&mut data)?;
    Ok(serde_json::from_str(&data)?)
}
