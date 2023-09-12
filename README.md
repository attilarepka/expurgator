# expurgator

expurgator is a Rust-based utility for efficiently cleaning and purging unwanted files from archive formats.

## Features

- Remove unwanted files from various archive formats using a CSV filter file.
- Supports `.zip`, `.tar`, `.tar.gz`, and `.tar.xz` archives.

## Installation

To use Expurgator, you'll need to have Rust installed on your system. If you don't have Rust installed, you can get it from [https://www.rust-lang.org/](https://www.rust-lang.org/).

Once Rust is installed, you can build Expurgator using Cargo, the Rust package manager:

```shell
git clone https://github.com/attilarepka/expurgator.git
cd expurgator
cargo build --release
```

## Usage

Expurgator provides a command-line interface with the following options:

```shell
Usage: expurgator [OPTIONS] --input-file <INPUT_FILE> --csv-file <CSV_FILE> --csv-index <CSV_INDEX>

Options:
  -i, --input-file <INPUT_FILE>
      Specify the input archive file.
  --csv-file <CSV_FILE>
      Specify the CSV file containing the list of files to be removed.
  -c, --csv-index <CSV_INDEX>
      Index of the field in the CSV containing the list of files to be removed.
  --has-header
      Specify this flag if the CSV contains a header record [default: false].
  -o, --output-file <OUTPUT_FILE>
      Specify the output file [default: --input-file].
  --compression-level <COMPRESSION_LEVEL>
      Set the compression level [default: 6].
  -h, --help
      Print help.
  -V, --version
      Print version.
```

## Contributing

Open a GitHub issue or pull request.

## License

This project is licensed under the [MIT license](LICENSE)
