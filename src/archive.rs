use std::{
    borrow::BorrowMut,
    io::{BufReader, BufWriter, Read, Write},
    path::PathBuf,
};

use anyhow::{anyhow, Result};
use bzip2::{read::BzDecoder, write::BzEncoder};
use flate2::{read::GzDecoder, write::GzEncoder};
use indicatif::ProgressBar;
use xz2::{read::XzDecoder, write::XzEncoder};
use zip::write::SimpleFileOptions;

use crate::util::{infer_input_file, prompt_error};

pub fn pack_archive(
    progress_bar: &ProgressBar,
    input: Vec<u8>,
    excluded_paths: &mut Vec<PathBuf>,
    compression_level: u32,
) -> Result<Vec<u8>> {
    let mime_type = infer_input_file(&input)?;
    match mime_type.as_str() {
        "application/zip" => encode_zip(progress_bar, input, excluded_paths, compression_level),
        "application/gzip" | "application/x-bzip2" | "application/x-xz" | "application/x-tar" => {
            encode_tar(
                progress_bar,
                &input,
                excluded_paths,
                compression_level,
                mime_type.as_str(),
            )
        }
        _ => Err(anyhow!(
            "Unsupported File Type: The file with MIME type '{}' is not supported.",
            mime_type
        ))?,
    }
}

trait WriteEncoder: Write {
    fn inner(self: Box<Self>) -> Result<Vec<u8>>;
}

impl WriteEncoder for GzEncoder<Vec<u8>> {
    fn inner(self: Box<Self>) -> Result<Vec<u8>> {
        Ok(self.finish()?)
    }
}

impl WriteEncoder for BzEncoder<Vec<u8>> {
    fn inner(self: Box<Self>) -> Result<Vec<u8>> {
        Ok(self.finish()?)
    }
}

impl WriteEncoder for XzEncoder<Vec<u8>> {
    fn inner(self: Box<Self>) -> Result<Vec<u8>> {
        Ok(self.finish()?)
    }
}

impl WriteEncoder for BufWriter<Vec<u8>> {
    fn inner(self: Box<Self>) -> Result<Vec<u8>> {
        Ok(self.into_inner()?)
    }
}

enum TarEncoder {
    Gzip(GzEncoder<Vec<u8>>),
    Bzip2(BzEncoder<Vec<u8>>),
    Xz2(XzEncoder<Vec<u8>>),
    XTar(BufWriter<Vec<u8>>),
}

impl TarEncoder {
    fn new(mime_type: &str, compression_level: u32) -> Result<Self> {
        match mime_type {
            "application/gzip" => {
                let result = GzEncoder::new(Vec::new(), flate2::Compression::new(compression_level));
                Ok(TarEncoder::Gzip(result))
            }
            "application/x-bzip2" => {
                let reuslt = BzEncoder::new(Vec::new(), bzip2::Compression::new(compression_level));
                Ok(TarEncoder::Bzip2(reuslt))
            }
            "application/x-xz" => {
                let result = XzEncoder::new(Vec::new(), compression_level);
                Ok(TarEncoder::Xz2(result))
            }
            "application/x-tar" => {
                let result = BufWriter::new(Vec::new());
                Ok(TarEncoder::XTar(result))
            }
            _ => Err(anyhow!("Unsupported Encoding Format: The provided MIME type does not correspond to a supported encoding format.")),
        }
    }

    fn encoder(self) -> Box<dyn WriteEncoder> {
        match self {
            TarEncoder::Gzip(result) => Box::new(result),
            TarEncoder::Bzip2(result) => Box::new(result),
            TarEncoder::Xz2(result) => Box::new(result),
            TarEncoder::XTar(result) => Box::new(result),
        }
    }
}

fn create_tar_decoder<'a>(reader: &'a [u8], mime_type: &str) -> Result<Box<dyn Read + 'a>> {
    match mime_type {
        "application/gzip" => {
            Ok(Box::new(GzDecoder::new(reader)))
        }
        "application/x-bzip2" => {
            Ok(Box::new(BzDecoder::new(reader)))
        }
        "application/x-xz" => {
            Ok(Box::new(XzDecoder::new(reader)))
        }
        "application/x-tar" => {
            Ok(Box::new(BufReader::new(reader)))
        }
        _ => Err(anyhow!("Unsupported Decoding Format: The provided MIME type does not correspond to a supported decoding format."))?,
    }
}

fn retain_inner_vec(input: &mut Vec<PathBuf>, filter: &str) -> Vec<PathBuf> {
    let mut inner_list = Vec::new();
    input.retain_mut(|e| {
        if e.starts_with(filter) {
            inner_list.push(std::mem::take(e));
            return false;
        }
        true
    });
    inner_list
}

fn zip_handle_inner_archive(
    progress_bar: &ProgressBar,
    entry_bytes: Vec<u8>,
    excluded_paths: &mut Vec<PathBuf>,
    compression_level: u32,
    path: &str,
    options: SimpleFileOptions,
    zip_writer: &mut zip::ZipWriter<std::io::Cursor<&mut Vec<u8>>>,
) -> Result<()> {
    let result = pack_archive(progress_bar, entry_bytes, excluded_paths, compression_level)?;
    zip_writer.start_file(path, options)?;
    zip_writer.write_all(&result)?;

    Ok(())
}

fn process_zip_entry(
    entry: &mut zip::read::ZipFile<std::io::Cursor<Vec<u8>>>,
    zip_writer: &mut zip::ZipWriter<std::io::Cursor<&mut Vec<u8>>>,
    excluded_paths: &mut Vec<PathBuf>,
    progress_bar: &ProgressBar,
    compression_level: u32,
) -> Result<()> {
    let path = entry.name().to_owned();
    let options = SimpleFileOptions::default()
        .compression_level(Some(compression_level.into()))
        .compression_method(entry.compression())
        .unix_permissions(entry.unix_mode().unwrap_or(0o777));

    progress_bar.set_message(format!("processing: {path}"));

    if let Some(found_file) = excluded_paths.iter().position(|e| e.ends_with(&path)) {
        excluded_paths.swap_remove(found_file);
    } else {
        if entry.is_dir() {
            zip_writer.add_directory(&path, options)?;
        }
        if entry.is_file() {
            let mut entry_bytes = vec![Default::default(); entry.size().try_into()?];
            entry.read_exact(&mut entry_bytes)?;

            if infer::is_archive(&entry_bytes) {
                progress_bar.set_message(format!("inner archive: {}", &path));
                let mut excluded_paths = retain_inner_vec(excluded_paths, &path);
                if !excluded_paths.is_empty() {
                    zip_handle_inner_archive(
                        progress_bar,
                        entry_bytes,
                        &mut excluded_paths,
                        compression_level,
                        path.as_str(),
                        options,
                        zip_writer,
                    )?;
                    return Ok(());
                }
            }
            zip_writer.start_file(path, options)?;
            zip_writer.write_all(&entry_bytes)?;
        }
    }
    Ok(())
}

fn encode_zip(
    progress_bar: &ProgressBar,
    input: Vec<u8>,
    excluded_paths: &mut Vec<PathBuf>,
    compression_level: u32,
) -> Result<Vec<u8>> {
    let decoder = std::io::Cursor::new(input);

    let mut zip_entries = zip::ZipArchive::new(decoder).unwrap();
    let mut result: Vec<u8> = Vec::new();
    {
        let encoder = std::io::Cursor::new(&mut result);
        let mut zip = zip::ZipWriter::new(encoder);

        for i in 0..zip_entries.len() {
            let mut entry = zip_entries.by_index(i)?;
            process_zip_entry(
                &mut entry,
                &mut zip,
                excluded_paths,
                progress_bar,
                compression_level,
            )?;
        }
        zip.finish()?;
    }
    Ok(result)
}

fn tar_handle_inner_archive(
    progress_bar: &ProgressBar,
    input: Vec<u8>,
    excluded_paths: &mut Vec<PathBuf>,
    path: &str,
    compression_level: u32,
) -> Result<(Vec<u8>, bool)> {
    if infer::is_archive(&input) {
        progress_bar.set_message(format!("inner archive: {path}"));
        let mut excluded_paths = retain_inner_vec(excluded_paths, path);
        if !excluded_paths.is_empty() {
            let result = pack_archive(progress_bar, input, &mut excluded_paths, compression_level)?;
            return Ok((result, true));
        }
    }
    Ok((input, false))
}

fn encode_tar(
    progress_bar: &ProgressBar,
    input: &[u8],
    excluded_paths: &mut Vec<PathBuf>,
    compression_level: u32,
    mime_type: &str,
) -> Result<Vec<u8>> {
    let decoder = create_tar_decoder(input, mime_type)?;
    let mut tar_archive = tar::Archive::new(decoder);

    let tar_encoder = TarEncoder::new(mime_type, compression_level).unwrap();
    let encoder = tar_encoder.encoder();
    let mut tar_writer = tar::Builder::new(encoder);
    for entry in tar_archive.entries()? {
        match entry {
            Ok(mut entry) => {
                let path = (*entry.path()?).to_owned();
                let path = path.to_string_lossy().to_string();
                progress_bar.set_message(format!("processing: {path}"));

                if let Some(found_file) = excluded_paths.iter().position(|e| e.ends_with(&path)) {
                    excluded_paths.swap_remove(found_file);
                } else {
                    match entry.header().entry_type() {
                        tar::EntryType::Directory => {
                            progress_bar.set_message(format!("adding directory: {path}"));
                            tar_writer.append_dir(&path, ".")?;
                        }
                        tar::EntryType::Regular
                        | tar::EntryType::GNUSparse
                        | tar::EntryType::Continuous
                        | tar::EntryType::Fifo
                        | tar::EntryType::Char
                        | tar::EntryType::Block
                        | tar::EntryType::GNULongName
                        | tar::EntryType::XGlobalHeader
                        | tar::EntryType::XHeader => {
                            progress_bar.set_message(format!("adding file: {path}"));

                            // read exactly the size of the current entry
                            let mut inner_entry =
                                vec![Default::default(); entry.header().size()?.try_into()?];
                            entry.read_exact(&mut inner_entry)?;

                            let (inner_entry, is_archive) = tar_handle_inner_archive(
                                progress_bar,
                                inner_entry,
                                excluded_paths,
                                &path,
                                compression_level,
                            )?;
                            let mut header = entry.header().clone();
                            if is_archive {
                                header.set_size(inner_entry.len().try_into()?);
                            }
                            tar_writer.append_data(&mut header, &path, &*inner_entry)?;
                        }
                        tar::EntryType::Symlink
                        | tar::EntryType::Link
                        | tar::EntryType::GNULongLink => {
                            progress_bar.set_message(format!("adding link: {path}"));
                            tar_writer.append_link(
                                entry.header().clone().borrow_mut(),
                                &path,
                                entry
                                    .header()
                                    .link_name()?
                                    .unwrap_or(entry.header().path()?),
                            )?;
                        }
                        _ => progress_bar.set_message(format!("unhandled type: {path}")),
                    }
                }
            }
            Err(_) => {
                prompt_error(progress_bar)?;
            }
        }
    }
    let encoder = tar_writer.into_inner()?;
    let result = encoder.inner().unwrap();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_tar_encoder() {
        assert!(TarEncoder::new("application/gzip", 6).is_ok());
        assert!(TarEncoder::new("application/x-bzip2", 6).is_ok());
        assert!(TarEncoder::new("application/x-xz", 6).is_ok());
        assert!(TarEncoder::new("application/x-tar", 6).is_ok());
        assert!(TarEncoder::new("invalid", 6).is_err());
    }

    #[test]
    fn test_create_tar_decoder() {
        let input = Vec::new();
        assert!(create_tar_decoder(&input, "application/gzip").is_ok());
        assert!(create_tar_decoder(&input, "application/x-bzip2").is_ok());
        assert!(create_tar_decoder(&input, "application/x-xz").is_ok());
        assert!(create_tar_decoder(&input, "application/x-tar").is_ok());
        assert!(create_tar_decoder(&input, "invalid").is_err());
    }

    #[test]
    fn test_retain_inner_vec() {
        let mut input = vec![
            PathBuf::from("1/2"),
            PathBuf::from("2/2"),
            PathBuf::from("3/3"),
            PathBuf::from("3/4"),
        ];

        let output = retain_inner_vec(&mut input, "3");

        assert_eq!(input.len(), 2);
        assert_eq!(input[0].to_str().unwrap(), "1/2");
        assert_eq!(input[1].to_str().unwrap(), "2/2");
        assert_eq!(output.len(), 2);
        assert_eq!(output[0].to_str().unwrap(), "3/3");
        assert_eq!(output[1].to_str().unwrap(), "3/4");
    }
}
