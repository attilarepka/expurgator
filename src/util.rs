use std::{
    collections::HashSet,
    fs::{create_dir_all, OpenOptions},
    io::{IsTerminal, Write},
    path::{Component, Path, PathBuf},
};

use anyhow::{anyhow, Result};
use csv::ReaderBuilder;
use indicatif::ProgressBar;
use inquire::Confirm;

pub fn file_to_bytes(file_path: &str) -> Result<Vec<u8>> {
    let bytes = std::fs::read(file_path)?;
    Ok(bytes)
}

pub fn parse_csv(file_path: &str, index: usize, header: bool) -> Result<HashSet<PathBuf>> {
    let mut reader = ReaderBuilder::new()
        .has_headers(header)
        .from_path(file_path)?;
    let mut result: HashSet<PathBuf> = HashSet::new();

    for record in reader.records() {
        if let Some(field) = record?.get(index - 1) {
            result.insert(field.into());
        } else {
            Err(anyhow!(
                "Index Not Found: The expected index '{index}' was not found."
            ))?;
        }
    }

    Ok(result)
}

pub fn apply_archive_root(paths: HashSet<PathBuf>, root: &Path) -> Result<HashSet<PathBuf>> {
    if root.as_os_str().is_empty()
        || root
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(anyhow!(
            "Archive root '{}' must be a relative path without '..'; use '.' for the archive root",
            root.display()
        ));
    }
    let root: PathBuf = root
        .components()
        .filter(|component| matches!(component, Component::Normal(_)))
        .collect();

    paths
        .into_iter()
        .map(|path| {
            if !path.has_root() {
                return Ok(path);
            }
            let relative = path.strip_prefix(Path::new("/"))?;
            if relative.as_os_str().is_empty()
                || relative
                    .components()
                    .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
                || !relative
                    .components()
                    .any(|component| matches!(component, Component::Normal(_)))
            {
                return Err(anyhow!(
                    "CSV path '{}' must name a path under the archive root without '..'",
                    path.display()
                ));
            }
            let relative: PathBuf = relative
                .components()
                .filter(|component| matches!(component, Component::Normal(_)))
                .collect();
            Ok(root.join(relative))
        })
        .collect()
}

pub fn parse_compression(compression_level: u32) -> Result<u32> {
    match compression_level {
        0..=9 => Ok(compression_level),
        _ => Err(anyhow!(
            "Invalid Compression Level: Please choose a compression between 0 and 9."
        ))?,
    }
}

pub fn prompt_csv(result: &HashSet<PathBuf>) -> Result<()> {
    let first_entry = result
        .iter()
        .next()
        .map_or_else(|| "(empty)".to_owned(), |path| path.display().to_string());
    let ans = Confirm::new("Is this correct?")
        .with_default(false)
        .with_help_message(
            format!(
                "File contains {} records, first value:\n{}",
                result.len(),
                first_entry
            )
            .as_str(),
        )
        .prompt();

    match ans {
        Ok(true) => Ok(()),
        Ok(false) => Err(anyhow!("Stopped by SIGNAL. Exiting.."))?,
        Err(err) => Err(err)?,
    }
}

pub fn prompt_error(progress_bar: &ProgressBar, error: std::io::Error) -> Result<()> {
    let mut ans = Ok(false);
    progress_bar.suspend(|| {
        ans = Confirm::new("Do you want to continue?")
            .with_default(false)
            .with_help_message("Failed to process tar entry, this data will be skipped")
            .prompt();
    });
    match ans {
        Ok(true) => Ok(()),
        Ok(false) => Err(anyhow!(
            "User interrupted; failed to process tar entry: {error}"
        )),
        Err(err) => Err(err)?,
    }
}

pub fn infer_input_file(file_bytes: &[u8]) -> Result<String> {
    if infer::is_archive(file_bytes) {
        let kind = infer::get(file_bytes);
        return Ok(kind.unwrap().mime_type().to_string());
    }
    Err(anyhow!(
        "Unsupported File Type: Only archive file types are supported."
    ))?
}

pub fn to_file(dst: &Path, payload: &[u8], force: bool) -> Result<()> {
    if let Some(parent) = dst.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        create_dir_all(parent)?;
    }

    let mut options = OpenOptions::new();
    options.write(true);
    if force {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }

    let mut file = match options.open(dst) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists && !force => {
            return Err(anyhow!(
                "Output file '{}' already exists; use --force to overwrite it",
                dst.display()
            ));
        }
        Err(err) => return Err(err.into()),
    };
    file.write_all(payload)?;

    Ok(())
}

pub fn is_interactive() -> bool {
    std::io::stdin().is_terminal()
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
        assert!(output.contains(&PathBuf::from("some/path")));
        assert!(output.contains(&PathBuf::from("some/other/path")));

        let output = parse_csv(file.path().to_str().unwrap(), 3, true).unwrap();
        assert_eq!(output.len(), 1);
        assert!(output.contains(&PathBuf::from("some/other/path")));

        assert!(parse_csv(file.path().to_str().unwrap(), 5, false).is_err());
    }

    #[test]
    fn archive_root_maps_only_leading_slash_paths() {
        let paths = HashSet::from([
            PathBuf::from("/acorn/srcrename"),
            PathBuf::from("old/root/file"),
        ]);
        let result = apply_archive_root(paths.clone(), Path::new("source root")).unwrap();
        assert!(result.contains(Path::new("source root/acorn/srcrename")));
        assert!(result.contains(Path::new("old/root/file")));

        let result = apply_archive_root(paths, Path::new(".")).unwrap();
        assert!(result.contains(Path::new("acorn/srcrename")));
        assert!(result.contains(Path::new("old/root/file")));
    }

    #[test]
    fn archive_root_rejects_escaping_or_empty_paths() {
        let paths = HashSet::from([PathBuf::from("/file")]);
        for root in ["", "/absolute", "../other", "root/../other"] {
            assert!(apply_archive_root(paths.clone(), Path::new(root)).is_err());
        }
        for path in ["/", "/.", "/../file", "/folder/../file"] {
            assert!(
                apply_archive_root(HashSet::from([PathBuf::from(path)]), Path::new("root"))
                    .is_err()
            );
        }
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
            file_to_bytes(file.path().to_str().unwrap()).unwrap(),
            payload.as_bytes()
        );

        assert!(file_to_bytes("nonexistent_file").is_err());
    }

    #[test]
    fn test_to_file_honors_path_and_prevents_overwrite_without_force() {
        let dir = assert_fs::TempDir::new().unwrap();
        let output = dir.path().join("artifacts/result.bin");

        to_file(&output, b"first", false).unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), b"first");

        let err = to_file(&output, b"second", false).unwrap_err();
        assert!(err.to_string().contains("--force"));
        assert_eq!(std::fs::read(&output).unwrap(), b"first");

        to_file(&output, b"second", true).unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), b"second");
    }
}
