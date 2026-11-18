use assert_cmd::Command;
use predicates::prelude::*;
use std::io::{Cursor, Read, Write};

#[test]
fn test_argument_help() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("expurgator")?;

    cmd.arg("--help");
    cmd.assert().success();

    Ok(())
}

#[test]
fn test_argument_version() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("expurgator")?;

    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("0.2.0"));

    Ok(())
}

#[test]
fn test_argument_missing_mandatory() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("expurgator")?;

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains(
            "error: the following required arguments were not provided:",
        ))
        .stderr(predicate::str::contains("<INPUT>"))
        .stderr(predicate::str::contains("<FILTER>"))
        .stderr(predicate::str::contains("--index <INDEX>"));

    Ok(())
}

#[test]
fn test_non_interactive_run_writes_to_explicit_output() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = assert_fs::TempDir::new()?;
    let output = temp_dir.path().join("artifacts/filtered.tar.gz");
    let mut cmd = Command::cargo_bin("expurgator")?;

    cmd.arg("--index")
        .arg("2")
        .arg("--non-interactive")
        .arg("--output")
        .arg(&output)
        .arg("tests/archives/tar-test.tar.gz")
        .arg("tests/assets/tar-test.csv")
        .assert()
        .success();

    assert!(output.is_file());
    Ok(())
}

#[test]
fn test_non_tty_run_does_not_prompt() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = assert_fs::TempDir::new()?;
    let output = temp_dir.path().join("filtered.tar.gz");
    let mut cmd = Command::cargo_bin("expurgator")?;

    cmd.arg("--index")
        .arg("2")
        .arg("--output")
        .arg(&output)
        .arg("tests/archives/tar-test.tar.gz")
        .arg("tests/assets/tar-test.csv")
        .assert()
        .success();

    assert!(output.is_file());
    Ok(())
}

#[test]
fn test_zip_preserves_entry_timestamps() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = assert_fs::TempDir::new()?;
    let input = temp_dir.path().join("input.zip");
    let filter = temp_dir.path().join("filter.csv");
    let output = temp_dir.path().join("output.zip");
    let modified = zip::DateTime::from_date_and_time(2001, 2, 3, 4, 5, 6)?;
    let options = zip::write::SimpleFileOptions::default().last_modified_time(modified);

    let mut nested = zip::ZipWriter::new(Cursor::new(Vec::new()));
    nested.start_file("keep.txt", options)?;
    nested.write_all(b"keep")?;
    nested.start_file("remove.txt", options)?;
    nested.write_all(b"remove")?;
    let nested = nested.finish()?.into_inner();

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.add_directory("folder/", options)?;
    zip.start_file("folder/file.txt", options)?;
    zip.write_all(b"keep")?;
    zip.start_file("nested.zip", options)?;
    zip.write_all(&nested)?;
    std::fs::write(&input, zip.finish()?.into_inner())?;
    std::fs::write(&filter, "nested.zip/remove.txt\n")?;

    Command::cargo_bin("expurgator")?
        .args(["--non-interactive", "--index", "1", "--output"])
        .arg(&output)
        .arg(&input)
        .arg(&filter)
        .assert()
        .success();

    let mut zip = zip::ZipArchive::new(std::fs::File::open(output)?)?;
    for path in ["folder/", "folder/file.txt", "nested.zip"] {
        assert_eq!(zip.by_name(path)?.last_modified(), Some(modified));
    }
    let mut nested_data = Vec::new();
    zip.by_name("nested.zip")?.read_to_end(&mut nested_data)?;
    let mut nested = zip::ZipArchive::new(Cursor::new(nested_data))?;
    assert_eq!(nested.by_name("keep.txt")?.last_modified(), Some(modified));
    assert!(nested.by_name("remove.txt").is_err());
    Ok(())
}

#[test]
fn test_invalid_zip_timestamp_fails_unless_entry_is_excluded(
) -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = assert_fs::TempDir::new()?;
    let input = temp_dir.path().join("input.zip");
    let filter = temp_dir.path().join("filter.csv");
    let output = temp_dir.path().join("output.zip");

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("entry.txt", zip::write::SimpleFileOptions::default())?;
    zip.write_all(b"content")?;
    let mut bytes = zip.finish()?.into_inner();
    let central = bytes
        .windows(4)
        .position(|window| window == b"PK\x01\x02")
        .expect("central directory header");
    bytes[12..14].copy_from_slice(&[0, 0]);
    bytes[central + 14..central + 16].copy_from_slice(&[0, 0]);
    std::fs::write(&input, bytes)?;
    std::fs::write(&filter, "other.txt\n")?;

    Command::cargo_bin("expurgator")?
        .args(["--non-interactive", "--index", "1", "--output"])
        .arg(&output)
        .arg(&input)
        .arg(&filter)
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "Invalid ZIP modification timestamp",
        ))
        .stderr(predicate::str::contains("entry.txt"));
    assert!(!output.exists());

    std::fs::write(&filter, "entry.txt\n")?;
    Command::cargo_bin("expurgator")?
        .args(["--non-interactive", "--index", "1", "--output"])
        .arg(&output)
        .arg(&input)
        .arg(&filter)
        .assert()
        .success();
    Ok(())
}

#[test]
fn test_invalid_archive_fails_without_panicking() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = assert_fs::TempDir::new()?;
    let input = temp_dir.path().join("invalid.zip");
    let output = temp_dir.path().join("output.zip");
    std::fs::write(&input, b"PK\x03\x04")?;
    let mut cmd = Command::cargo_bin("expurgator")?;

    cmd.arg("--index")
        .arg("1")
        .arg("--non-interactive")
        .arg("--output")
        .arg(&output)
        .arg(&input)
        .arg("tests/assets/tar-test.csv")
        .assert()
        .failure()
        .stderr(predicates::str::contains("Error:"))
        .stderr(predicates::str::contains("panicked").not());

    assert!(!output.exists());
    Ok(())
}

#[test]
fn test_unsupported_tar_entry_fails_unless_excluded() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = assert_fs::TempDir::new()?;
    let input = temp_dir.path().join("input.tar.gz");
    let filter = temp_dir.path().join("filter.csv");
    let output = temp_dir.path().join("output.tar.gz");

    let mut header = tar::Header::new_gnu();
    header.set_entry_type(tar::EntryType::new(b'Z'));
    header.set_mode(0o644);
    header.set_size(0);
    header.set_cksum();
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    builder.append_data(&mut header, "unknown-entry", std::io::empty())?;
    let archive = builder.into_inner()?.finish()?;
    std::fs::write(&input, archive)?;
    std::fs::write(&filter, "other-entry\n")?;

    Command::cargo_bin("expurgator")?
        .args(["--non-interactive", "--index", "1", "--output"])
        .arg(&output)
        .arg(&input)
        .arg(&filter)
        .assert()
        .failure()
        .stderr(predicate::str::contains("unknown-entry"))
        .stderr(predicate::str::contains("unsupported tar entry type"));
    assert!(!output.exists());

    std::fs::write(&filter, "unknown-entry\n")?;
    Command::cargo_bin("expurgator")?
        .args(["--non-interactive", "--index", "1", "--output"])
        .arg(&output)
        .arg(&input)
        .arg(&filter)
        .assert()
        .success();
    assert!(output.exists());
    Ok(())
}

#[test]
fn test_exact_paths_and_duplicates_in_nested_archives() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = assert_fs::TempDir::new()?;
    let input = temp_dir.path().join("input.tar.gz");
    let filter = temp_dir.path().join("filter.csv");
    let output = temp_dir.path().join("output.tar.gz");

    let mut inner_zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for path in ["foo/bar.txt", "other/foo/bar.txt", "foo/keep.txt"] {
        inner_zip.start_file(path, zip::write::SimpleFileOptions::default())?;
        inner_zip.write_all(b"content")?;
    }
    let inner_zip = inner_zip.finish()?.into_inner();

    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    for (path, contents) in [
        ("foo/bar.txt", &b"first"[..]),
        ("other/foo/bar.txt", &b"keep"[..]),
        ("foo/bar.txt", &b"second"[..]),
        ("foo/keep.txt", &b"keep"[..]),
        ("nested.zip", inner_zip.as_slice()),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, path, contents)?;
    }
    std::fs::write(&input, builder.into_inner()?.finish()?)?;
    std::fs::write(&filter, "foo/bar.txt\nnested.zip/foo/bar.txt\n")?;

    Command::cargo_bin("expurgator")?
        .args(["--non-interactive", "--index", "1", "--output"])
        .arg(&output)
        .arg(&input)
        .arg(&filter)
        .assert()
        .success();

    let archive = std::fs::File::open(output)?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    let mut remaining = Vec::new();
    let mut nested_data = Vec::new();
    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        if path == std::path::Path::new("nested.zip") {
            entry.read_to_end(&mut nested_data)?;
        }
        remaining.push(path);
    }
    assert_eq!(
        remaining,
        [
            std::path::PathBuf::from("other/foo/bar.txt"),
            std::path::PathBuf::from("foo/keep.txt"),
            std::path::PathBuf::from("nested.zip"),
        ]
    );

    let mut nested = zip::ZipArchive::new(Cursor::new(nested_data))?;
    assert_eq!(nested.len(), 2);
    assert!(nested.by_name("foo/bar.txt").is_err());
    assert!(nested.by_name("other/foo/bar.txt").is_ok());
    assert!(nested.by_name("foo/keep.txt").is_ok());
    Ok(())
}

#[test]
fn test_directory_exclusions_in_tar_and_nested_zip() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = assert_fs::TempDir::new()?;
    let input = temp_dir.path().join("input.tar.gz");
    let filter = temp_dir.path().join("filter.csv");
    let output = temp_dir.path().join("output.tar.gz");

    let mut inner_zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    inner_zip.add_directory("folder/", zip::write::SimpleFileOptions::default())?;
    for path in ["folder/file.txt", "folder/sub/file.txt", "folder2/keep.txt"] {
        inner_zip.start_file(path, zip::write::SimpleFileOptions::default())?;
        inner_zip.write_all(b"content")?;
    }
    let inner_zip = inner_zip.finish()?.into_inner();

    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    for (path, contents) in [
        ("folder/file.txt", &b"remove"[..]),
        ("folder/sub/file.txt", &b"remove"[..]),
        ("folder2/keep.txt", &b"keep"[..]),
        ("nested.zip", inner_zip.as_slice()),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, path, contents)?;
    }
    std::fs::write(&input, builder.into_inner()?.finish()?)?;
    std::fs::write(&filter, "folder/\nnested.zip/folder\n")?;

    Command::cargo_bin("expurgator")?
        .args(["--non-interactive", "--index", "1", "--output"])
        .arg(&output)
        .arg(&input)
        .arg(&filter)
        .assert()
        .success();

    let archive = std::fs::File::open(output)?;
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    let mut remaining = Vec::new();
    let mut nested_data = Vec::new();
    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        if path == std::path::Path::new("nested.zip") {
            entry.read_to_end(&mut nested_data)?;
        }
        remaining.push(path);
    }
    assert_eq!(
        remaining,
        [
            std::path::PathBuf::from("folder2/keep.txt"),
            std::path::PathBuf::from("nested.zip"),
        ]
    );

    let mut nested = zip::ZipArchive::new(Cursor::new(nested_data))?;
    assert_eq!(nested.len(), 1);
    assert!(nested.by_name("folder2/keep.txt").is_ok());
    Ok(())
}

#[test]
fn test_directory_exclusions_in_zip() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = assert_fs::TempDir::new()?;
    let input = temp_dir.path().join("input.zip");
    let filter = temp_dir.path().join("filter.csv");
    let output = temp_dir.path().join("output.zip");

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.add_directory("folder/", zip::write::SimpleFileOptions::default())?;
    for path in ["folder/file.txt", "folder/sub/file.txt", "folder2/keep.txt"] {
        zip.start_file(path, zip::write::SimpleFileOptions::default())?;
        zip.write_all(b"content")?;
    }
    std::fs::write(&input, zip.finish()?.into_inner())?;
    std::fs::write(&filter, "folder\n")?;

    Command::cargo_bin("expurgator")?
        .args(["--non-interactive", "--index", "1", "--output"])
        .arg(&output)
        .arg(&input)
        .arg(&filter)
        .assert()
        .success();

    let mut zip = zip::ZipArchive::new(std::fs::File::open(output)?)?;
    assert_eq!(zip.len(), 1);
    assert!(zip.by_name("folder2/keep.txt").is_ok());
    Ok(())
}

#[test]
fn test_duplicate_zip_entries_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = assert_fs::TempDir::new()?;
    let input = temp_dir.path().join("input.zip");
    let filter = temp_dir.path().join("filter.csv");
    let output = temp_dir.path().join("output.zip");

    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for path in [
        "foo/bar.txt",
        "other/foo/bar.txt",
        "foo/baz.txt",
        "keep.txt",
    ] {
        zip.start_file(path, zip::write::SimpleFileOptions::default())?;
        zip.write_all(b"content")?;
    }
    let mut bytes = zip.finish()?.into_inner();
    // The ZIP writer rejects duplicate names, so rename both headers of the last entry.
    let mut replacements = 0;
    for position in 0..=bytes.len() - b"foo/baz.txt".len() {
        if bytes[position..].starts_with(b"foo/baz.txt") {
            bytes[position..position + b"foo/bar.txt".len()].copy_from_slice(b"foo/bar.txt");
            replacements += 1;
        }
    }
    assert_eq!(replacements, 2);
    assert!(zip::ZipArchive::new(Cursor::new(&bytes))?.has_duplicate_names());
    std::fs::write(&input, &bytes)?;

    for filter_path in ["foo/bar.txt", "keep.txt"] {
        std::fs::write(&filter, format!("{filter_path}\n"))?;
        Command::cargo_bin("expurgator")?
            .args(["--non-interactive", "--index", "1", "--output"])
            .arg(&output)
            .arg(&input)
            .arg(&filter)
            .assert()
            .failure()
            .stderr(predicate::str::contains("duplicate entry names"));
        assert!(!output.exists());
    }

    let outer_input = temp_dir.path().join("outer.tar.gz");
    let outer_output = temp_dir.path().join("outer-output.tar.gz");
    let encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    let mut header = tar::Header::new_gnu();
    header.set_size(bytes.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append_data(&mut header, "nested.zip", bytes.as_slice())?;
    std::fs::write(&outer_input, builder.into_inner()?.finish()?)?;
    std::fs::write(&filter, "nested.zip/foo/bar.txt\n")?;
    Command::cargo_bin("expurgator")?
        .args(["--non-interactive", "--index", "1", "--output"])
        .arg(&outer_output)
        .arg(&outer_input)
        .arg(&filter)
        .assert()
        .failure()
        .stderr(predicate::str::contains("duplicate entry names"));
    assert!(!outer_output.exists());
    Ok(())
}
