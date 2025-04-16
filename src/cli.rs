use clap::{ArgAction, Parser};

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
    #[arg(long, short)]
    pub index: usize,

    /// Specify this flag if the CSV contains a header record [default: false]
    #[arg(long, short, action=ArgAction::SetFalse)]
    pub with_headers: bool,

    /// Output file [default: --input]
    #[arg(long, short)]
    pub output: Option<String>,

    /// Compression level
    #[arg(long, short, default_value_t = 6)]
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
