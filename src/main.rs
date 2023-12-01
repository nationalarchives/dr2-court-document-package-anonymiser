use anonymise::*;
use clap::Parser;
use log::{self, LevelFilter};
use simple_logger::SimpleLogger;
use std::{path::PathBuf, process::exit};

struct Files {
    dir_output: PathBuf,
    files: Vec<PathBuf>,
}
fn files_from_input_arguments(opt: Opt) -> Files {
    let dir_input: PathBuf = PathBuf::from(shellexpand::full(&opt.input).unwrap().to_string());
    let dir_output: PathBuf = PathBuf::from(shellexpand::full(&opt.output).unwrap().to_string());
    let files = files_in_input_dir(&dir_input).unwrap();
    Files { dir_output, files }
}
fn main() {
    SimpleLogger::new()
        .with_level(LevelFilter::Info)
        .init()
        .unwrap();
    let files_from_input = files_from_input_arguments(Opt::parse());
    for file in files_from_input.files {
        match process_package(&files_from_input.dir_output, &file) {
            Ok(_) => {
                log::info!(
                    "Processed {}",
                    file.file_name().and_then(|name| name.to_str()).unwrap()
                )
            }
            Err(err) => {
                log::error!("Error: {:?}", err);
                exit(1);
            }
        };
    }
}

#[cfg(test)]
mod test {
    use crate::files_from_input_arguments;
    use anonymise::Opt;
    use assert_fs::TempDir;
    use std::fs::write;
    use std::path::{Path, PathBuf};

    #[test]
    fn test_files_from_input_arguments() {
        let input_dir = TempDir::new().unwrap();
        let test_file_names = ["file1", "file2", "file3"];
        let _ = test_file_names.map(|file_name| {
            let file_path = input_dir.join(PathBuf::from(file_name));
            write(file_path.clone(), "".as_bytes()).unwrap();
            file_path
        });
        let input = input_dir.to_str().unwrap().to_string();
        let output = TempDir::new().unwrap().to_str().unwrap().to_string();
        let opt = Opt { input, output };
        let files_result = files_from_input_arguments(opt);
        let files = files_result.files;

        fn get_file_name(file_path: &Path) -> &str {
            file_path
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .unwrap()
        }

        assert_eq!(files.len(), 3);
        assert_eq!(get_file_name(&files[0]), "file3");
        assert_eq!(get_file_name(&files[1]), "file2");
        assert_eq!(get_file_name(&files[2]), "file1")
    }
}
