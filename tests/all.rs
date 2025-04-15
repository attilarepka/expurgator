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
        .stdout(predicate::str::contains("0.1.4"));

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
        .stderr(predicate::str::contains("--input <INPUT>"))
        .stderr(predicate::str::contains("--csv <CSV>"))
        .stderr(predicate::str::contains("--index <INDEX>"));

    Ok(())
}

#[ignore]
#[test]
fn test_extract_tar_gz() -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::cargo_bin("expurgator")?;

    cmd.arg("--input")
        .arg("tests/archives/tar-test.tar.gz")
        .arg("--csv")
        .arg("tests/assets/tar-test.csv")
        .arg("--index")
        .arg("2")
        .write_stdin("y\n")
        .assert()
        .success();

    Ok(())
}
