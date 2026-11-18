use assert_cmd::Command;
use predicates::prelude::*;

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
