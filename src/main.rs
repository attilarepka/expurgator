use bzip2::read::BzDecoder;
use bzip2::write::BzEncoder;
use clap::Parser;
use csv::ReaderBuilder;
use flate2;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use inquire::Confirm;
use std::borrow::BorrowMut;
use std::error::Error;
use std::fs::*;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Read;
use std::io::Write;
use std::path::*;
use std::time::Duration;
use tar;
use xz2::read::XzDecoder;
use xz2::write::XzEncoder;
use zip::write::FileOptions;

#[derive(Parser, Debug, Clone)]
#[command(author, version, about = "expurgator", long_about = None)]
struct Args {
    /// Input archive file
    #[clap(long, short)]
    input_file: String,

    /// CSV file containing the list of files to be removed
    #[clap(long)]
    csv_file: String,

    /// Index of the field in CSV containing the list of files to be removed
    #[clap(long)]
    csv_index: usize,

    /// Specify this flag if the CSV contains a header record [default: false]
    #[clap(long)]
    has_header: bool,

    /// Output file [default: --input-file]
    #[clap(long, short)]
    output_file: Option<String>,

    /// Compression level
    #[clap(long, default_value = "6")]
    compression_level: u32,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let compression_level = parse_compression_level(args.compression_level)?;

    let input_bytes = get_file_as_byte_vec(&args.input_file)?;

    let mut filter_list = parse_csv_file(&args.csv_file, args.csv_index, args.has_header)?;
    filter_list = prompt_csv_record(filter_list)?;

    let progress_bar = ProgressBar::new_spinner();
    progress_bar.enable_steady_tick(Duration::from_millis(120));
    progress_bar.set_style(
        ProgressStyle::with_template("{spinner:.blue} {msg}")
            .unwrap()
            .tick_strings(&[
                "▹▹▹▹▹",
                "▸▹▹▹▹",
                "▹▸▹▹▹",
                "▹▹▸▹▹",
                "▹▹▹▸▹",
                "▹▹▹▹▸",
                "▪▪▪▪▪",
            ]),
    );

    let result_bytes = pack(
        &progress_bar,
        input_bytes,
        &mut filter_list,
        compression_level,
    )?;

    write_file(
        args.output_file.unwrap_or(args.input_file).as_str(),
        result_bytes,
    )?;

    Ok(())
}

fn pack(
    progress_bar: &ProgressBar,
    input_bytes: Vec<u8>,
    filter_list: &mut Vec<PathBuf>,
    compression_level: u32,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mime_type = infer_input_file(&input_bytes)?;
    match mime_type.as_str() {
        "application/zip" => pack_zip(&progress_bar, input_bytes, filter_list, compression_level),
        "application/gzip" | "application/x-bzip2" | "application/x-xz" | "application/x-tar" => {
            pack_tar(
                &progress_bar,
                input_bytes,
                filter_list,
                compression_level,
                mime_type.as_str(),
            )
        }
        _ => Err(format!(
            "Unsupported File Type: The file with MIME type '{}' is not supported.",
            mime_type
        ))?,
    }
}

fn parse_compression_level(compression_level: u32) -> Result<u32, Box<dyn Error>> {
    match compression_level {
        0..=9 => Ok(compression_level),
        _ => Err(
            "Invalid Compression Level: Please choose a compression_level level between 0 and 9.",
        )?,
    }
}

fn prompt_csv_record(result: Vec<PathBuf>) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let ans = Confirm::new("Is this correct?")
        .with_default(false)
        .with_help_message(
            format!(
                "CSV contains {} records, the first index has the value of:\n{}",
                result.len(),
                result.first().unwrap().display(),
            )
            .as_str(),
        )
        .prompt();

    match ans {
        Ok(true) => return Ok(result),
        Ok(false) => Err("User Interruption: The process has been interrupted. Exiting...")?,
        Err(err) => Err(err)?,
    };

    Ok(result)
}

fn parse_csv_file(
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
            result.push(field.try_into()?);
        } else {
            Err(format!(
                "Index Not Found: The expected index '{}' was not found.",
                index
            ))?;
        }
    }

    Ok(result)
}

fn infer_input_file(file_bytes: &[u8]) -> Result<String, Box<dyn Error>> {
    if infer::is_archive(&file_bytes) {
        let kind = infer::get(&file_bytes);
        return Ok(kind.unwrap().mime_type().to_string());
    }
    Err("Unsupported File Type: Only archive file types are supported.")?
}

trait WriteEncoder: Write {
    fn inner(self: Box<Self>) -> Result<Vec<u8>, Box<dyn Error>>;
}

impl WriteEncoder for GzEncoder<Vec<u8>> {
    fn inner(self: Box<Self>) -> Result<Vec<u8>, Box<dyn Error>> {
        Ok(self.finish()?)
    }
}

impl WriteEncoder for BzEncoder<Vec<u8>> {
    fn inner(self: Box<Self>) -> Result<Vec<u8>, Box<dyn Error>> {
        Ok(self.finish()?)
    }
}

impl WriteEncoder for XzEncoder<Vec<u8>> {
    fn inner(self: Box<Self>) -> Result<Vec<u8>, Box<dyn Error>> {
        Ok(self.finish()?)
    }
}

impl WriteEncoder for BufWriter<Vec<u8>> {
    fn inner(self: Box<Self>) -> Result<Vec<u8>, Box<dyn Error>> {
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
    fn new(mime_type: &str, compression_level: u32) -> Result<Self, Box<dyn Error>> {
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
            _ => Err(format!("Unsupported Encoding Format: The provided MIME type does not correspond to a supported encoding format.").into()),
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

fn create_tar_decoder<'a>(
    reader: &'a [u8],
    mime_type: &str,
) -> Result<Box<dyn Read + 'a>, Box<dyn Error>> {
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
        _ => Err("Unsupported Decoding Format: The provided MIME type does not correspond to a supported decoding format.")?,
    }
}

fn get_file_as_byte_vec(file_path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let bytes = std::fs::read(&file_path)?;
    Ok(bytes)
}

fn retain_inner_vec(
    input: &mut Vec<PathBuf>,
    filter: &str,
) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut inner_list = Vec::new();
    input.retain_mut(|e| {
        if e.starts_with(&filter) {
            inner_list.push(std::mem::take(e));
            return false;
        }
        true
    });
    Ok(inner_list)
}

fn zip_handle_inner_archive(
    progress_bar: &ProgressBar,
    entry_bytes: Vec<u8>,
    mut filter_list: &mut Vec<PathBuf>,
    compression_level: u32,
    path: &str,
    options: FileOptions,
    zip_writer: &mut zip::ZipWriter<std::io::Cursor<&mut Vec<u8>>>,
) -> Result<(), Box<dyn Error>> {
    let result = pack(
        progress_bar,
        entry_bytes,
        &mut filter_list,
        compression_level,
    )?;
    zip_writer.start_file(path, options)?;
    zip_writer.write_all(&*result)?;

    Ok(())
}

fn process_zip_entry(
    entry: &mut zip::read::ZipFile,
    zip_writer: &mut zip::ZipWriter<std::io::Cursor<&mut Vec<u8>>>,
    filter_list: &mut Vec<PathBuf>,
    progress_bar: &ProgressBar,
    compression_level: u32,
) -> Result<(), Box<dyn Error>> {
    let path = entry.name().to_owned();
    let options = FileOptions::default()
        .compression_level(Some(compression_level.try_into()?))
        .compression_method(entry.compression())
        .unix_permissions(entry.unix_mode().unwrap_or(0o777));

    progress_bar.set_message(format!("processing: {}", path));

    if let Some(found_file) = filter_list.iter().position(|e| e.ends_with(&path)) {
        filter_list.swap_remove(found_file);
    } else {
        if entry.is_dir() {
            zip_writer.add_directory(&path, options)?;
        }
        if entry.is_file() {
            let mut entry_bytes = vec![Default::default(); entry.size().try_into()?];
            entry.read_exact(&mut entry_bytes)?;

            if infer::is_archive(&entry_bytes) {
                progress_bar.set_message(format!("inner archive: {}", &path));
                let mut inner_filter_list = retain_inner_vec(filter_list, &path)?;
                if inner_filter_list.len() > 0 {
                    zip_handle_inner_archive(
                        progress_bar,
                        entry_bytes,
                        &mut inner_filter_list,
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

fn pack_zip(
    progress_bar: &ProgressBar,
    input_bytes: Vec<u8>,
    filter_list: &mut Vec<PathBuf>,
    compression_level: u32,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let decoder = std::io::Cursor::new(input_bytes);

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
                filter_list,
                progress_bar,
                compression_level,
            )?;
        }
        zip.finish()?;
    }
    Ok(result)
}

fn prompt_error(progress_bar: &ProgressBar) -> Result<(), Box<dyn Error>> {
    let mut ans = Ok(false);
    progress_bar.suspend(|| {
        ans = Confirm::new("Do you want to continue?")
            .with_default(false)
            .with_help_message("Failed to process tar entry, this data will be skipped")
            .prompt();
    });
    match ans {
        Ok(true) => return Ok(()),
        Ok(false) => return Err("User interrupted, exiting")?,
        Err(err) => return Err(err)?,
    }
}

fn tar_handle_inner_archive(
    progress_bar: &ProgressBar,
    input_bytes: Vec<u8>,
    filter_list: &mut Vec<PathBuf>,
    path: &str,
    compression_level: u32,
) -> Result<(Vec<u8>, bool), Box<dyn Error>> {
    if infer::is_archive(&input_bytes) {
        progress_bar.set_message(format!("inner archive: {}", path));
        let mut inner_filter_list = retain_inner_vec(filter_list, &path)?;
        if inner_filter_list.len() > 0 {
            let inner_entry_bytes = pack(
                progress_bar,
                input_bytes,
                &mut inner_filter_list,
                compression_level,
            )?;
            return Ok((inner_entry_bytes, true));
        }
    }
    Ok((input_bytes, false))
}

fn pack_tar(
    progress_bar: &ProgressBar,
    input_bytes: Vec<u8>,
    filter_list: &mut Vec<PathBuf>,
    compression_level: u32,
    mime_type: &str,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let decoder = create_tar_decoder(&*input_bytes, mime_type)?;
    let mut tar_archive = tar::Archive::new(decoder);

    let tar_encoder = TarEncoder::new(mime_type, compression_level).unwrap();
    let encoder = tar_encoder.encoder();
    let mut tar_writer = tar::Builder::new(encoder);
    for entry in tar_archive.entries()? {
        match entry {
            Ok(mut entry) => {
                let path = (*entry.path()?).to_owned();
                let path = path.to_string_lossy().to_string();
                progress_bar.set_message(format!("processing: {}", path));

                if let Some(found_file) = filter_list.iter().position(|e| e.ends_with(&path)) {
                    filter_list.swap_remove(found_file);
                } else {
                    match entry.header().entry_type() {
                        tar::EntryType::Directory => {
                            progress_bar.set_message(format!("adding directory: {}", path));
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
                            progress_bar.set_message(format!("adding file: {}", path));

                            // read exactly the size of the current entry
                            let mut inner_entry =
                                vec![Default::default(); entry.header().size()?.try_into()?];
                            entry.read_exact(&mut inner_entry)?;

                            let (inner_entry, is_archive) = tar_handle_inner_archive(
                                progress_bar,
                                inner_entry,
                                filter_list,
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
                            progress_bar.set_message(format!("adding link: {}", path));
                            tar_writer.append_link(
                                entry.header().clone().borrow_mut(),
                                &path,
                                &entry
                                    .header()
                                    .link_name()?
                                    .unwrap_or(entry.header().path()?),
                            )?;
                        }
                        _ => progress_bar.set_message(format!("unhandled type: {}", path)),
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

fn write_file(dst: &str, payload: Vec<u8>) -> Result<(), Box<dyn Error>> {
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
        assert_eq!(parse_compression_level(5).unwrap(), 5);
        assert!(parse_compression_level(42).is_err());
    }

    #[test]
    fn test_parse_csv_file() {
        let file = assert_fs::NamedTempFile::new("input.csv").unwrap();
        file.write_str("1,2,some/path,4\n1,2,some/other/path,4")
            .unwrap();
        let output = parse_csv_file(file.path().to_str().unwrap(), 3, false).unwrap();
        assert_eq!(output.len(), 2);
        assert_eq!(output[0].to_str().unwrap(), "some/path");
        assert_eq!(output[1].to_str().unwrap(), "some/other/path");

        let output = parse_csv_file(file.path().to_str().unwrap(), 3, true).unwrap();
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].to_str().unwrap(), "some/other/path");

        assert!(parse_csv_file(file.path().to_str().unwrap(), 5, false).is_err());
    }

    #[test]
    fn test_infer_input_file() {
        let buf = [0x50, 0x4B, 0x3, 0x4];
        assert_eq!(infer_input_file(&buf).unwrap(), "application/zip");

        let buf = [0xFF, 0xD8, 0xFF, 0xAA];
        assert!(infer_input_file(&buf).is_err());
    }

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
    fn test_get_file_as_byte_vec() {
        let payload = "abcd";
        let file = assert_fs::NamedTempFile::new("input.csv").unwrap();
        file.write_str(payload).unwrap();

        assert_eq!(
            get_file_as_byte_vec(file.path().to_str().unwrap()).unwrap(),
            payload.as_bytes()
        );

        assert!(get_file_as_byte_vec("nonexistent_file").is_err());
    }

    #[test]
    fn test_retain_inner_vec() {
        let mut input = Vec::new();
        input.push(PathBuf::from("1/2"));
        input.push(PathBuf::from("2/2"));
        input.push(PathBuf::from("3/3"));
        input.push(PathBuf::from("3/4"));

        let output = retain_inner_vec(&mut input, "3").unwrap();

        assert_eq!(input.len(), 2);
        assert_eq!(input[0].to_str().unwrap(), "1/2");
        assert_eq!(input[1].to_str().unwrap(), "2/2");
        assert_eq!(output.len(), 2);
        assert_eq!(output[0].to_str().unwrap(), "3/3");
        assert_eq!(output[1].to_str().unwrap(), "3/4");
    }
}
