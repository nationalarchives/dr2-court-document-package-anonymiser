use std::{
    fs,
    fs::File,
    io::Write,
    path::PathBuf,
    io::Read,
    path::Path
};
use std::fs::DirEntry;
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
    let mut opt = Opt::parse();
    let dir_input = PathBuf::from(shellexpand::full(&opt.input).unwrap().to_string());
    let dir_output = PathBuf::from(shellexpand::full(&opt.output).unwrap().to_string());
    let files = list_files_in_input_dir(&dir_input).unwrap();
    for file in files {
        let tar_gz_file_name = file.file_name()
            .and_then(|name| name.to_os_string().into_string().ok())
            .unwrap();

        let output_tar_gz_path: PathBuf = Path::new(&dir_output).join(Path::new(&tar_gz_file_name.replace("TDR","TST")));
        let uncompressed_folder_input_path = &file.with_extension("").with_extension("");
        let input_batch_reference = uncompressed_folder_input_path.clone().file_name().unwrap().to_str().unwrap().to_string();
        let output_batch_reference = &input_batch_reference.replace("TDR", "TST");

        let extracted_output_original_name = dir_output.join(PathBuf::from(&input_batch_reference));
        let extracted_output_path = dir_output.join(PathBuf::from(output_batch_reference));

        fs::create_dir_all(extracted_output_path.clone()).expect("Could not create directory in output folder");

        decompress_file(&file, &dir_output)
            .expect("Error decompressing file");

        let metadata_input_file_path = &extracted_output_path.join(PathBuf::from(format!("TRE-{}-metadata.json", &input_batch_reference)));
        let metadata_output_file_path = &extracted_output_path.join(PathBuf::from(format!("TRE-{}-metadata.json", &output_batch_reference)));

        if extracted_output_path.exists() {
            fs::remove_dir_all(&extracted_output_path).unwrap();
        }
        fs::rename(&extracted_output_original_name, &extracted_output_path).unwrap();
        fs::rename(&metadata_input_file_path, &metadata_output_file_path).unwrap();

        let mut json_value = parse_json(&metadata_output_file_path)
            .expect("Error parsing json");

        let docx_file_name = json_value["parameters"]["TRE"]["payload"]["filename"].as_str()
            .expect("Filename is missing from the metadata json");
        let judgment_name = json_value["parameters"]["PARSER"]["name"].to_string();
        let docx_path_string = extracted_output_path.join(PathBuf::from(docx_file_name));
        println!();

        create_docx(&docx_path_string, judgment_name)
            .expect("Error creating the docx file");

        let checksum = checksum(&docx_path_string);

        update_json_file(&metadata_output_file_path, checksum, &mut json_value).expect("Error updating the json file");

        fs::remove_file(extracted_output_path.join(PathBuf::from(format!("{}.xml", input_batch_reference).as_str()))).unwrap();
        fs::remove_file(extracted_output_path.join(PathBuf::from("parser.log"))).unwrap();

        tar_folder(&output_tar_gz_path, &extracted_output_path, &output_batch_reference).expect("Error creating the tar file");

        fs::remove_dir_all(&extracted_output_path).unwrap();
    }

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
fn list_files_in_input_dir(directory_path: &PathBuf) -> Result<Vec<PathBuf>, std::io::Error> {
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

fn tar_folder(tar_path: &PathBuf, path_to_compress: &PathBuf, folder_name: &String) -> Result<(), std::io::Error> {
    let tar_gz = File::create(tar_path)?;
    let enc = GzEncoder::new(tar_gz, Compression::default());
    let mut tar = tar::Builder::new(enc);
    tar.append_dir_all(folder_name, &path_to_compress)?;
    Ok(())
}

fn create_docx(docx_path: &PathBuf, judgment_name: String) -> Result<(), std::io::Error> {
    let file = File::create(&docx_path)?;
    Docx::new()
        .add_paragraph(Paragraph::new().add_run(Run::new().add_text(judgment_name)))
        .build()
        .pack(file)?;
    Ok(())
}

fn update_json_file(metadata_file_name: &PathBuf, checksum: String, json_value: &mut Value) -> Result<(), std::io::Error> {
    json_value["parameters"]["TDR"]["Contact-Email"] = json!("XXXXXXXXX");
    json_value["parameters"]["TDR"]["Contact-Name"] = json!("XXXXXXXXX");
    json_value["parameters"]["TDR"]["Document-Checksum-sha256"] = json!(checksum);
    fs::write(metadata_file_name, json_value.to_string())
}

fn decompress_file(path_to_tar: &PathBuf, output_path: &PathBuf) -> Result<(), std::io::Error> {
    let tar_gz = File::open(path_to_tar)?;
    let tar = GzDecoder::new(tar_gz);
    let mut archive = Archive::new(tar);
    archive.unpack(&output_path)?;
    Ok(())
}
fn parse_json(metadata_file_path: &PathBuf) -> Result<Value, std::io::Error> {
    let mut file = File::open(metadata_file_path)?;
    let mut data = String::new();
    file.read_to_string(&mut data)?;
    Ok(serde_json::from_str(&data)?)
}
