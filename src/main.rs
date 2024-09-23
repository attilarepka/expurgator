mod archive;
mod cli;
mod util;

use archive::pack_archive;
use indicatif::ProgressBar;
use indicatif::ProgressStyle;
use std::error::Error;
use std::time::Duration;
use util::parse_compression;
use util::parse_csv;
use util::prompt_csv;
use util::to_bytes;
use util::to_file;

fn main() -> Result<(), Box<dyn Error>> {
    let args = cli::Args::from();

    let compression_level = parse_compression(args.compression)?;

    let input_bytes = to_bytes(&args.input)?;

    let mut filter_list = parse_csv(&args.csv, args.index, args.with_headers)?;
    prompt_csv(&filter_list)?;

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

    let result_bytes = pack_archive(
        &progress_bar,
        input_bytes,
        &mut filter_list,
        compression_level,
    )?;

    to_file(args.output.unwrap().as_str(), result_bytes)?;

    Ok(())
}
