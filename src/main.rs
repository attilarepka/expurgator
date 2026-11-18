mod archive;
mod cli;
mod util;

use anyhow::Result;
use archive::pack_archive;
use clap::Parser;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::Path;
use std::time::Duration;
use util::{file_to_bytes, is_interactive, parse_compression, parse_csv, prompt_csv, to_file};

fn main() -> Result<()> {
    let args = cli::Args::parse();

    let compression_level = parse_compression(args.compression)?;

    let input = file_to_bytes(&args.input)?;

    let excluded_paths = parse_csv(&args.filter, args.index, args.with_headers)?;
    let non_interactive = args.non_interactive || !is_interactive();
    if !non_interactive {
        prompt_csv(&excluded_paths)?;
    }

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

    let result = pack_archive(
        &progress_bar,
        input,
        &excluded_paths,
        compression_level,
        non_interactive,
    )?;

    let output = match args.output {
        Some(output) => output.into(),
        None => Path::new("out").join(
            Path::new(&args.input)
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Input path '{}' has no filename", args.input))?,
        ),
    };
    to_file(&output, &result, args.force)?;

    Ok(())
}
