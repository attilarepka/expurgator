use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about = None, long_about = None)]
pub struct Args {
    /// Input archive file
    #[arg(index = 1)]
    pub input: String,

    /// CSV file specifying which files or paths should be removed from the input archive
    #[arg(index = 2)]
    pub filter: String,

    /// Index of the CSV column specifying which files or paths should be removed from the input archive
    #[arg(long, short, value_parser = parse_positive_index)]
    pub index: usize,

    /// Specify this flag if the CSV contains a header record [default: false]
    #[arg(long, short)]
    pub with_headers: bool,

    /// Output file [default: out/<input filename>]
    #[arg(long, short)]
    pub output: Option<String>,

    /// Compression level
    #[arg(long, short, default_value_t = 6)]
    pub compression: u32,

    /// Do not prompt; malformed archive entries cause an error
    #[arg(long)]
    pub non_interactive: bool,

    /// Overwrite the output file if it already exists
    #[arg(long, short)]
    pub force: bool,
}

fn parse_positive_index(value: &str) -> Result<usize, String> {
    let index = value
        .parse::<usize>()
        .map_err(|error| format!("invalid index '{value}': {error}"))?;
    if index == 0 {
        return Err("index must be greater than zero".to_owned());
    }
    Ok(index)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_flag_enables_headers() {
        let args = Args::try_parse_from(["expurgator", "input.zip", "filter.csv", "-i", "1", "-w"])
            .unwrap();
        assert!(args.with_headers);
    }

    #[test]
    fn index_must_be_positive() {
        assert!(
            Args::try_parse_from(["expurgator", "input.zip", "filter.csv", "-i", "0"]).is_err()
        );
    }
}
