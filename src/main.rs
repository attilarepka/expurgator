use bzip2::read::BzDecoder;
use bzip2::write::BzEncoder;
use clap::Parser;
use csv::ReaderBuilder;
use flate2;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use inquire::Confirm;
use std::borrow::BorrowMut;
use std::error::Error;
use std::fs::*;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Read;
use std::io::Write;
use std::path::*;
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
    #[clap(long, short)]
    csv_index: usize,

    /// Specify this flag if the CSV contains a header record [default: false]
    #[clap(long)]
    has_header: bool,

    /// Output file [default: --input-file]
    #[clap(long, short)]
    output_file: Option<String>,

    /// Compression level
    #[clap(long, default_value = "6")]
    compression_level: usize,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();

    let compression = parse_compression_level(args.compression_level)?;

    let archive_vec = get_file_as_byte_vec(&args.input_file)?;

    let unwanted_files = parse_csv_file(&args.csv_file, args.csv_index, args.has_header)?;

    let result_bytes = pack(archive_vec, unwanted_files, compression)?;

    write_file(args.output_file.unwrap_or(args.input_file), result_bytes)?;

    Ok(())
}

fn pack(
    archive_vec: Vec<u8>,
    unwanted_files: Vec<PathBuf>,
    compression: usize,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let mime_type = infer_input_file(&archive_vec)?;
    match mime_type.as_str() {
        "application/zip" => pack_zip(archive_vec, unwanted_files, compression),
        "application/gzip" | "application/x-bzip2" | "application/x-xz" | "application/x-tar" => {
            pack_archive(archive_vec, unwanted_files, compression, mime_type)
        }
        _ => Err(format!(
            "Unsupported File Type: The file with MIME type '{}' is not supported.",
            mime_type
        ))?,
    }
}

fn parse_compression_level(compression: usize) -> Result<usize, Box<dyn Error>> {
    match compression {
        0..=9 => Ok(compression),
        _ => Err("Invalid Compression Level: Please choose a compression level between 0 and 9.")?,
    }
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
        let record = record?;
        if let Some(field) = record.get(index - 1) {
            result.push(PathBuf::from(field));
        } else {
            Err(format!(
                "Index Not Found: The expected index '{}' was not found.",
                index
            ))?;
        }
    }

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

fn infer_input_file(file_bytes: &[u8]) -> Result<String, Box<dyn Error>> {
    if infer::is_archive(&file_bytes) {
        let kind = infer::get(&file_bytes);
        return Ok(kind.unwrap().mime_type().to_string());
    }
    Err("Unsupported File Type: Only archive file types are supported.")?
}

fn create_tar_encoder<'a>(
    archive_vec: &'a mut Vec<u8>,
    mime_type: &str,
) -> Result<Box<dyn Write + 'a>, Box<dyn Error>> {
    match mime_type {
        "application/gzip" => {
            let writer = Box::new(GzEncoder::new(archive_vec, flate2::Compression::default()));
            Ok(writer)
        }
        "application/x-bzip2" => {
            let writer = Box::new(BzEncoder::new(archive_vec, bzip2::Compression::default()));
            Ok(writer)
        }
        "application/x-xz" => {
            let writer = Box::new(XzEncoder::new(archive_vec, 6));
            Ok(writer)
        }
        "application/x-tar" => {
            let writer = Box::new(BufWriter::new(archive_vec));
            Ok(writer)
        }
        _ => Err("Unsupported Encoding Format: The provided MIME type does not correspond to a supported encoding format.")?,
    }
}

fn create_tar_decoder<'a>(
    archive_vec: &'a [u8],
    mime_type: &str,
) -> Result<Box<dyn Read + 'a>, Box<dyn Error>> {
    match mime_type {
        "application/gzip" => {
            let reader = Box::new(GzDecoder::new(archive_vec));
            Ok(reader)
        }
        "application/x-bzip2" => {
            let reader = Box::new(BzDecoder::new(archive_vec));
            Ok(reader)
        }
        "application/x-xz" => {
            let reader = Box::new(XzDecoder::new(archive_vec));
            Ok(reader)
        }
        "application/x-tar" => {
            let reader = Box::new(BufReader::new(archive_vec));
            Ok(reader)
        }
        _ => Err("Unsupported Decoding Format: The provided MIME type does not correspond to a supported decoding format.")?,
    }
}

fn get_file_as_byte_vec(file_path: &str) -> Result<Vec<u8>, Box<dyn Error>> {
    let bytes = std::fs::read(&file_path)?;
    Ok(bytes)
}

fn pack_zip(
    archive_vec: Vec<u8>,
    mut unwanted_files: Vec<PathBuf>,
    compression: usize,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let decoder = std::io::Cursor::new(&*archive_vec);

    let mut zip_entries = zip::ZipArchive::new(decoder).unwrap();
    let mut result: Vec<u8> = Vec::new();
    {
        let encoder = std::io::Cursor::new(&mut result);
        let mut zip = zip::ZipWriter::new(encoder);

        for i in 0..zip_entries.len() {
            let mut entry = zip_entries.by_index(i)?;
            let path = String::from(entry.name());
            let options = FileOptions::default()
                .compression_method(entry.compression())
                .unix_permissions(entry.unix_mode().unwrap_or(0o777));

            println!("processing: {}", path);

            if let Some(found_file) = unwanted_files.iter().position(|e| e.ends_with(&path)) {
                unwanted_files.swap_remove(found_file);
            } else {
                if entry.is_dir() {
                    zip.add_directory(path, options)?;
                    continue;
                }
                if entry.is_file() {
                    let mut entry_bytes = vec![Default::default(); entry.size().try_into()?];
                    entry.read_exact(&mut entry_bytes)?;

                    if infer::is_archive(&entry_bytes) {
                        println!("inner archive: {}", path);
                        let (inner_vec, old_vec): (Vec<_>, Vec<_>) = unwanted_files
                            .into_iter()
                            .partition(|e| e.starts_with(&path));
                        unwanted_files = old_vec;
                        if inner_vec.len() > 0 {
                            let archive_vec = pack(entry_bytes, inner_vec, compression)?;
                            zip.start_file(path, options)?;
                            zip.write_all(&*archive_vec)?;

                            continue;
                        }
                    }
                    zip.start_file(path, options)?;
                    zip.write_all(&entry_bytes)?;

                    continue;
                }
            }
        }
        zip.finish()?;
    }
    Ok(result)
}

fn pack_archive(
    archive_vec: Vec<u8>,
    mut unwanted_files: Vec<PathBuf>,
    compression: usize,
    mime_type: String,
) -> Result<Vec<u8>, Box<dyn Error>> {
    let decoder = create_tar_decoder(&*archive_vec, mime_type.as_str())?;

    let mut tar_archive = tar::Archive::new(decoder);
    let tar_entries = tar_archive.entries()?;

    let mut result: Vec<u8> = Vec::new();
    {
        let encoder = create_tar_encoder(&mut result, mime_type.as_str())?;

        let mut tar = tar::Builder::new(encoder);

        for entry in tar_entries {
            let mut entry = match entry {
                Err(err) => {
                    println!("Error: {}", err);
                    let ans = Confirm::new("Do you want to continue?")
                        .with_default(false)
                        .with_help_message("Failed to process tar entry, this data will be skipped")
                        .prompt();

                    match ans {
                        Ok(true) => continue,
                        Ok(false) => Err(err)?,
                        Err(err) => Err(err)?,
                    }
                }
                Ok(res) => res,
            };

            let path = entry.path()?.into_owned();
            println!("processing: {}", path.display());

            if let Some(found_file) = unwanted_files.iter().position(|e| e.ends_with(&path)) {
                unwanted_files.swap_remove(found_file);
            } else {
                match entry.header().entry_type() {
                    tar::EntryType::Directory => {
                        println!("adding directory {}", path.display());
                        tar.append_dir(&path, ".")?;
                    }
                    tar::EntryType::Regular
                    | tar::EntryType::GNUSparse // nope tar-rs not yet supporting this properly
                    | tar::EntryType::Continuous
                    | tar::EntryType::Fifo
                    | tar::EntryType::Char
                    | tar::EntryType::Block
                    | tar::EntryType::GNULongName
                    | tar::EntryType::XGlobalHeader
                    | tar::EntryType::XHeader => {
                        println!("adding file {}", path.display());

                        // read exactly the size of the current entry
                        let mut entry_bytes =
                            vec![Default::default(); entry.header().size()?.try_into()?];
                        entry.read_exact(&mut entry_bytes)?;

                        if infer::is_archive(&entry_bytes) {
                            println!("inner archive: {}", path.display());
                            let (inner_vec, old_vec): (Vec<_>, Vec<_>) = unwanted_files
                                .into_iter()
                                .partition(|e| e.starts_with(&path));
                            unwanted_files = old_vec;
                            if inner_vec.len() > 0 {
                                let archive_vec = pack(entry_bytes, inner_vec, compression)?;
                                // header size needs correction as we removed few files
                                let mut header = entry.header().clone();
                                header.set_size(archive_vec.len().try_into()?);
                                tar.append_data(&mut header, &path, &*archive_vec)?;
                                continue;
                            }
                        }
                        tar.append_data(entry.header().clone().borrow_mut(), &path, &*entry_bytes)?;
                    }
                    tar::EntryType::Symlink
                    | tar::EntryType::Link
                    | tar::EntryType::GNULongLink => {
                        println!("adding link {}", path.display());
                        tar.append_link(
                            entry.header().clone().borrow_mut(),
                            &path,
                            &entry
                                .header()
                                .link_name()?
                                .unwrap_or(entry.header().path()?),
                        )?;
                    }
                    _ => println!("unhandled type {}", path.display()),
                }
            }
        }
        tar.finish()?;
    }
    Ok(result)
}

fn write_file(dst: String, payload: Vec<u8>) -> Result<(), Box<dyn Error>> {
    let mut out = String::from("out/");
    if !Path::new(out.as_str()).exists() {
        create_dir_all(out.as_str())?;
    }
    out.push_str(dst.as_str());
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(out)?;

    file.write_all(&payload)?;

    Ok(())
}

