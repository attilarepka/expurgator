use std::{
    collections::HashSet,
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{anyhow, Result};
use bzip2::{read::BzDecoder, write::BzEncoder};
use flate2::{read::GzDecoder, write::GzEncoder};
use indicatif::ProgressBar;
use xz2::{read::XzDecoder, write::XzEncoder};
use zip::write::SimpleFileOptions;

use crate::util::{infer_input_file, prompt_error};

#[derive(Clone, Copy)]
struct PackOptions {
    compression_level: u32,
    non_interactive: bool,
}

pub fn pack_archive(
    progress_bar: &ProgressBar,
    input: Vec<u8>,
    excluded_paths: &HashSet<PathBuf>,
    compression_level: u32,
    non_interactive: bool,
) -> Result<Vec<u8>> {
    let options = PackOptions {
        compression_level,
        non_interactive,
    };
    pack_archive_with_options(progress_bar, input, excluded_paths, &options)
}

fn pack_archive_with_options(
    progress_bar: &ProgressBar,
    input: Vec<u8>,
    excluded_paths: &HashSet<PathBuf>,
    options: &PackOptions,
) -> Result<Vec<u8>> {
    let mime_type = infer_input_file(&input)?;
    match mime_type.as_str() {
        "application/zip" => encode_zip(progress_bar, input, excluded_paths, options),
        "application/gzip" | "application/x-bzip2" | "application/x-xz" | "application/x-tar" => {
            encode_tar(
                progress_bar,
                &input,
                excluded_paths,
                mime_type.as_str(),
                options,
            )
        }
        _ => Err(anyhow!(
            "Unsupported File Type: The file with MIME type '{mime_type}' is not supported."
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

fn new_tar_encoder(mime_type: &str, compression_level: u32) -> Result<Box<dyn WriteEncoder>> {
    match mime_type {
        "application/gzip" => Ok(Box::new(GzEncoder::new(Vec::new(), flate2::Compression::new(compression_level)))),
        "application/x-bzip2" => Ok(Box::new(BzEncoder::new(Vec::new(), bzip2::Compression::new(compression_level)))),
        "application/x-xz" => Ok(Box::new(XzEncoder::new(Vec::new(), compression_level))),
        "application/x-tar" => Ok(Box::new(BufWriter::new(Vec::new()))),
        _ => Err(anyhow!("Unsupported Encoding Format: The provided MIME type does not correspond to a supported encoding format.")),
    }
}

fn new_tar_decoder<'a>(reader: &'a [u8], mime_type: &str) -> Result<Box<dyn Read + 'a>> {
    match mime_type {
        "application/gzip" => Ok(Box::new(GzDecoder::new(reader))),
        "application/x-bzip2" => Ok(Box::new(BzDecoder::new(reader))),
        "application/x-xz" => Ok(Box::new(XzDecoder::new(reader))),
        "application/x-tar" => Ok(Box::new(BufReader::new(reader))),
        _ => Err(anyhow!("Unsupported Decoding Format: The provided MIME type does not correspond to a supported decoding format.")),
    }
}

fn nested_exclusions(excluded_paths: &HashSet<PathBuf>, path: &str) -> HashSet<PathBuf> {
    excluded_paths
        .iter()
        .filter_map(|excluded| excluded.strip_prefix(path).ok())
        .filter(|relative| !relative.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .collect()
}

fn is_excluded(excluded_paths: &HashSet<PathBuf>, path: &Path) -> bool {
    path.ancestors()
        .filter(|ancestor| !ancestor.as_os_str().is_empty())
        .any(|ancestor| excluded_paths.contains(ancestor))
}

fn zip_handle_inner_archive(
    progress_bar: &ProgressBar,
    entry_bytes: Vec<u8>,
    excluded_paths: &HashSet<PathBuf>,
    path: &str,
    options: SimpleFileOptions,
    zip_writer: &mut zip::ZipWriter<std::io::Cursor<&mut Vec<u8>>>,
    pack_options: &PackOptions,
) -> Result<()> {
    let result =
        pack_archive_with_options(progress_bar, entry_bytes, excluded_paths, pack_options)?;
    zip_writer.start_file(path, options)?;
    zip_writer.write_all(&result)?;

    Ok(())
}

fn process_zip_entry(
    entry: &mut zip::read::ZipFile<std::io::Cursor<Vec<u8>>>,
    zip_writer: &mut zip::ZipWriter<std::io::Cursor<&mut Vec<u8>>>,
    excluded_paths: &HashSet<PathBuf>,
    progress_bar: &ProgressBar,
    pack_options: &PackOptions,
) -> Result<()> {
    let path = entry.name()?.into_owned();
    progress_bar.set_message(format!("processing: {path}"));

    if !is_excluded(excluded_paths, Path::new(&path)) {
        let modified = entry
            .last_modified()
            .ok_or_else(|| anyhow!("Invalid ZIP modification timestamp for '{path}'"))?;
        let options = SimpleFileOptions::default()
            .compression_level(match entry.compression() {
                zip::CompressionMethod::Stored => None,
                _ => Some(pack_options.compression_level.into()),
            })
            .compression_method(entry.compression())
            .unix_permissions(entry.unix_mode().unwrap_or(0o777))
            .last_modified_time(modified);

        if entry.is_dir() {
            zip_writer.add_directory(&path, options)?;
        }
        if entry.is_file() {
            let mut entry_bytes = vec![Default::default(); entry.size().try_into()?];
            entry.read_exact(&mut entry_bytes)?;

            if infer::is_archive(&entry_bytes) {
                progress_bar.set_message(format!("inner archive: {path}"));
                let inner_exclusions = nested_exclusions(excluded_paths, &path);
                if !inner_exclusions.is_empty() {
                    zip_handle_inner_archive(
                        progress_bar,
                        entry_bytes,
                        &inner_exclusions,
                        path.as_str(),
                        options,
                        zip_writer,
                        pack_options,
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
    excluded_paths: &HashSet<PathBuf>,
    options: &PackOptions,
) -> Result<Vec<u8>> {
    let decoder = std::io::Cursor::new(input);

    let mut zip_entries = zip::ZipArchive::new(decoder)?;
    if zip_entries.has_duplicate_names() {
        return Err(anyhow!(
            "ZIP archives with duplicate entry names are not supported"
        ));
    }
    let mut result: Vec<u8> = Vec::new();
    {
        let encoder = std::io::Cursor::new(&mut result);
        let mut zip = zip::ZipWriter::new(encoder);

        for i in 0..zip_entries.len() {
            let mut entry = zip_entries.by_index(i)?;
            process_zip_entry(&mut entry, &mut zip, excluded_paths, progress_bar, options)?;
        }
        zip.finish()?;
    }
    Ok(result)
}

fn tar_handle_inner_archive(
    progress_bar: &ProgressBar,
    input: Vec<u8>,
    excluded_paths: &HashSet<PathBuf>,
    path: &str,
    options: &PackOptions,
) -> Result<(Vec<u8>, bool)> {
    if infer::is_archive(&input) {
        progress_bar.set_message(format!("inner archive: {path}"));
        let inner_exclusions = nested_exclusions(excluded_paths, path);
        if !inner_exclusions.is_empty() {
            let result =
                pack_archive_with_options(progress_bar, input, &inner_exclusions, options)?;
            return Ok((result, true));
        }
    }
    Ok((input, false))
}

fn is_file_entry_type(entry_type: tar::EntryType) -> bool {
    matches!(
        entry_type,
        tar::EntryType::Regular
            | tar::EntryType::GNUSparse
            | tar::EntryType::Continuous
            | tar::EntryType::Fifo
            | tar::EntryType::Char
            | tar::EntryType::Block
            | tar::EntryType::GNULongName
            | tar::EntryType::XGlobalHeader
            | tar::EntryType::XHeader
    )
}

fn is_link_entry_type(entry_type: tar::EntryType) -> bool {
    matches!(
        entry_type,
        tar::EntryType::Symlink | tar::EntryType::Link | tar::EntryType::GNULongLink
    )
}

fn encode_tar(
    progress_bar: &ProgressBar,
    input: &[u8],
    excluded_paths: &HashSet<PathBuf>,
    mime_type: &str,
    options: &PackOptions,
) -> Result<Vec<u8>> {
    let decoder = new_tar_decoder(input, mime_type)?;
    let mut tar_archive = tar::Archive::new(decoder);

    let encoder = new_tar_encoder(mime_type, options.compression_level)?;
    let mut tar_writer = tar::Builder::new(encoder);
    for entry in tar_archive.entries()? {
        match entry {
            Ok(mut entry) => {
                let path = (*entry.path()?).to_string_lossy().to_string();
                progress_bar.set_message(format!("processing: {path}"));

                if !is_excluded(excluded_paths, Path::new(&path)) {
                    match entry.header().entry_type() {
                        tar::EntryType::Directory => {
                            progress_bar.set_message(format!("adding directory: {path}"));
                            let mut header = entry.header().clone();
                            tar_writer.append_data(&mut header, &path, std::io::empty())?;
                        }
                        entry_type if is_file_entry_type(entry_type) => {
                            progress_bar.set_message(format!("adding file: {path}"));
                            let mut inner_entry =
                                vec![Default::default(); entry.header().size()?.try_into()?];
                            entry.read_exact(&mut inner_entry)?;

                            let (inner_entry, is_archive) = tar_handle_inner_archive(
                                progress_bar,
                                inner_entry,
                                excluded_paths,
                                &path,
                                options,
                            )?;
                            let mut header = entry.header().clone();
                            if is_archive {
                                header.set_size(inner_entry.len().try_into()?);
                            }
                            tar_writer.append_data(&mut header, &path, &*inner_entry)?;
                        }
                        entry_type if is_link_entry_type(entry_type) => {
                            progress_bar.set_message(format!("adding link: {path}"));
                            let mut header = entry.header().clone();
                            tar_writer.append_link(
                                &mut header,
                                &path,
                                entry
                                    .header()
                                    .link_name()?
                                    .unwrap_or(entry.header().path()?),
                            )?;
                        }
                        entry_type => {
                            return Err(anyhow!(
                                "unsupported tar entry type {entry_type:?} for '{path}'"
                            ));
                        }
                    }
                }
            }
            Err(error) => {
                if options.non_interactive {
                    return Err(anyhow!("Failed to process tar entry: {error}"));
                }
                prompt_error(progress_bar, error)?;
            }
        }
    }
    let encoder = tar_writer.into_inner()?;
    let result = encoder.inner()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tar_directory_preserves_header_metadata() -> Result<()> {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Directory);
        header.set_size(0);
        header.set_mode(0o751);
        header.set_uid(1234);
        header.set_gid(5678);
        header.set_mtime(1_234_567_890);
        header.set_username("archive-owner")?;
        header.set_groupname("archive-group")?;
        header.set_cksum();

        let mut builder = tar::Builder::new(Vec::new());
        builder.append_data(&mut header, "example/", std::io::empty())?;
        let input = builder.into_inner()?;
        let options = PackOptions {
            compression_level: 6,
            non_interactive: true,
        };
        let output = encode_tar(
            &ProgressBar::hidden(),
            &input,
            &HashSet::new(),
            "application/x-tar",
            &options,
        )?;

        let mut archive = tar::Archive::new(output.as_slice());
        let mut entries = archive.entries()?;
        let entry = entries.next().expect("directory entry")?;
        assert_eq!(entry.path()?.as_ref(), Path::new("example/"));
        let header = entry.header();
        assert_eq!(header.entry_type(), tar::EntryType::Directory);
        assert_eq!(header.mode()?, 0o751);
        assert_eq!(header.uid()?, 1234);
        assert_eq!(header.gid()?, 5678);
        assert_eq!(header.mtime()?, 1_234_567_890);
        assert_eq!(header.username()?, Some("archive-owner"));
        assert_eq!(header.groupname()?, Some("archive-group"));
        assert!(entries.next().is_none());
        Ok(())
    }

    #[test]
    fn test_create_tar_encoder() {
        assert!(new_tar_encoder("application/gzip", 6).is_ok());
        assert!(new_tar_encoder("application/x-bzip2", 6).is_ok());
        assert!(new_tar_encoder("application/x-xz", 6).is_ok());
        assert!(new_tar_encoder("application/x-tar", 6).is_ok());
        assert!(new_tar_encoder("invalid", 6).is_err());
    }

    #[test]
    fn test_create_tar_decoder() {
        let input = Vec::new();
        assert!(new_tar_decoder(&input, "application/gzip").is_ok());
        assert!(new_tar_decoder(&input, "application/x-bzip2").is_ok());
        assert!(new_tar_decoder(&input, "application/x-xz").is_ok());
        assert!(new_tar_decoder(&input, "application/x-tar").is_ok());
        assert!(new_tar_decoder(&input, "invalid").is_err());
    }
}
