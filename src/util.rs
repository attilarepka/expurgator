use std::{
    error::Error,
    fs::{create_dir_all, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

use csv::ReaderBuilder;
use indicatif::ProgressBar;
use inquire::Confirm;

pub fn to_bytes(file_path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let bytes = std::fs::read(file_path)?;
    Ok(bytes)
}

pub fn parse_csv(
    file_path: &str,
    index: usize,
    header: bool,
) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut reader = ReaderBuilder::new()
        .has_headers(header)
        .from_path(file_path)?;
    let mut result: Vec<PathBuf> = Vec::new();

    for record in reader.records() {
        if let Some(field) = record?.get(index - 1) {
            result.push(field.into());
        } else {
            Err(format!(
                "Index Not Found: The expected index '{}' was not found.",
                index
            ))?;
        }
    }

    Ok(result)
}

pub fn parse_compression(compression_level: u32) -> Result<u32, Box<dyn Error>> {
    match compression_level {
        0..=9 => Ok(compression_level),
        _ => Err("Invalid Compression Level: Please choose a compression between 0 and 9.")?,
    }
}

pub fn prompt_csv(result: &[PathBuf]) -> Result<(), Box<dyn Error>> {
    let ans = Confirm::new("Is this correct?")
        .with_default(false)
        .with_help_message(
            format!(
                "File contains {} records, first value:\n{}",
                result.len(),
                result.first().unwrap().display(),
            )
            .as_str(),
        )
        .prompt();

    match ans {
        Ok(true) => Ok(()),
        Ok(false) => Err("Stopped by SIGNAL. Exiting..")?,
        Err(err) => Err(err)?,
    }
}

pub fn prompt_error(progress_bar: &ProgressBar) -> Result<(), Box<dyn Error>> {
    let mut ans = Ok(false);
    progress_bar.suspend(|| {
        ans = Confirm::new("Do you want to continue?")
            .with_default(false)
            .with_help_message("Failed to process tar entry, this data will be skipped")
            .prompt();
    });
    match ans {
        Ok(true) => Ok(()),
        Ok(false) => Err("User interrupted, exiting")?,
        Err(err) => Err(err)?,
    }
}

pub fn infer_input_file(file_bytes: &[u8]) -> Result<String, Box<dyn Error>> {
    if infer::is_archive(file_bytes) {
        let kind = infer::get(file_bytes);
        return Ok(kind.unwrap().mime_type().to_string());
    }
    Err("Unsupported File Type: Only archive file types are supported.")?
}

pub fn to_file(dst: &str, payload: Vec<u8>) -> Result<(), Box<dyn Error>> {
    let mut out = String::from("out/");
    if !Path::new(out.as_str()).exists() {
        create_dir_all(out.as_str())?;
    }
    let out_path = Path::new(dst);
    out.push_str(out_path.file_name().unwrap().to_str().unwrap());
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(out)?;

    file.write_all(&payload)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::FileWriteStr;

    #[test]
    fn test_parse_compression_level() {
        assert_eq!(parse_compression(5).unwrap(), 5);
        assert!(parse_compression(42).is_err());
    }

    #[test]
    fn test_parse_csv_file() {
        let file = assert_fs::NamedTempFile::new("input.csv").unwrap();
        file.write_str("1,2,some/path,4\n1,2,some/other/path,4")
            .unwrap();
        let output = parse_csv(file.path().to_str().unwrap(), 3, false).unwrap();
        assert_eq!(output.len(), 2);
        assert_eq!(output[0].to_str().unwrap(), "some/path");
        assert_eq!(output[1].to_str().unwrap(), "some/other/path");

        let output = parse_csv(file.path().to_str().unwrap(), 3, true).unwrap();
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].to_str().unwrap(), "some/other/path");

        assert!(parse_csv(file.path().to_str().unwrap(), 5, false).is_err());
    }

    #[test]
    fn test_infer_input_file() {
        let buf = [0x50, 0x4B, 0x3, 0x4];
        assert_eq!(infer_input_file(&buf).unwrap(), "application/zip");

        let buf = [0xFF, 0xD8, 0xFF, 0xAA];
        assert!(infer_input_file(&buf).is_err());
    }

    #[test]
    fn test_get_file_as_byte_vec() {
        let payload = "abcd";
        let file = assert_fs::NamedTempFile::new("input.csv").unwrap();
        file.write_str(payload).unwrap();

        assert_eq!(
            to_bytes(file.path().to_str().unwrap()).unwrap(),
            payload.as_bytes()
        );

        assert!(to_bytes("nonexistent_file").is_err());
    }
}
