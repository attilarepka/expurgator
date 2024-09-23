use clap::{ArgAction, Parser};

#[derive(Parser, Debug)]
#[command(author, version, about = "expurgator", long_about = None)]
pub struct Args {
    /// Input archive file
    #[arg(long, short)]
    pub input: String,

    /// CSV file containing the list of files to be removed
    #[arg(long)]
    pub csv: String,

    /// Index of the field in CSV containing the list of files to be removed
    #[arg(long)]
    pub index: usize,

    /// Specify this flag if the CSV contains a header record [default: false]
    #[arg(long, action=ArgAction::SetFalse)]
    pub with_headers: bool,

    /// Output file [default: --input]
    #[arg(long, short)]
    pub output: Option<String>,

    /// Compression level
    #[arg(long, default_value_t = 6)]
    pub compression: u32,
}

impl Args {
    pub fn from() -> Args {
        let mut args = Args::parse();

        if args.output.is_none() {
            args.output = Some(args.input.clone());
        }

        args
    }
}
