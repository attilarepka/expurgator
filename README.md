# expurgator

expurgator is a cli utility for efficiently purging unwanted files from archive formats.

[![Build status](https://github.com/attilarepka/expurgator/actions/workflows/tests.yml/badge.svg)](https://github.com/attilarepka/expurgator/actions)

## Features

- Remove unwanted files from various archive formats using a CSV filter file.
- Supports `.zip`, `.tar`, `.tar.gz`, and `.tar.xz` archives.

## Installation

**[Archives of precompiled binaries for expurgator are available for 
macOS and Linux.](https://github.com/attilarepka/expurgator/releases)**

Linux binaries are static executables.

If you're a **Debian** user (or a user of a Debian derivative like **Ubuntu**),
then expurgator can be installed using a binary `.deb` file provided in each
[expurgator release](https://github.com/attilarepka/expurgator/releases).

```
$ curl -LO https://github.com/attilarepka/expurgator/releases/download/0.1.0/expurgator_0.1.0_amd64.deb
$ sudo dpkg -i expurgator_0.1.0_amd64.deb
```

### Building

expurgator is written in Rust, so you'll need [Rust installation](https://www.rust-lang.org/) in order to compile it.
expurgator compiles with Rust 1.70.0 (stable) or newer. In general, it tracks
the latest stable release of the Rust compiler.

```shell
$ git clone https://github.com/attilarepka/expurgator.git
$ cd expurgator
$ cargo build --release
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
  --csv-index <CSV_INDEX>
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
