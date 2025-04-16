mod archive;
mod cli;
mod util;

use anyhow::Result;
use archive::pack_archive;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use std::time::Duration;
use util::{file_to_bytes, parse_compression, parse_csv, prompt_csv, to_file};

fn main() -> Result<()> {
    let args = cli::Args::from();

    let compression_level = parse_compression(args.compression)?;

    let input = file_to_bytes(&args.input)?;

    let mut excluded_paths = parse_csv(&args.filter, args.index, args.with_headers)?;
    prompt_csv(&excluded_paths)?;

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

    let result = pack_archive(&progress_bar, input, &mut excluded_paths, compression_level)?;

    to_file(args.output.unwrap().as_str(), result)?;

    Ok(())
}
