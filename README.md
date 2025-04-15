# expurgator

expurgator is a CLI utility for removing files from archive formats based on a CSV file.

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
$ curl -LO https://github.com/attilarepka/expurgator/releases/download/0.1.4/expurgator_0.1.4_amd64.deb
$ sudo dpkg -i expurgator_0.1.4_amd64.deb
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
Usage: expurgator [OPTIONS] --input <INPUT> --csv <CSV> --index <INDEX>

Options:
  -i, --input <INPUT>
      Specify the input archive file.
  --csv <CSV>
      Specify the CSV file containing the list of files to be removed.
  --index <INDEX>
      Index of the field in the CSV containing the list of files to be removed.
  --with-headers
      Specify this flag if the CSV contains a header record [default: false].
  -o, --output <OUTPUT>
      Specify the output file [default: --input-file].
  --compression <COMPRESSION>
      Set the compression level [default: 6].
  -h, --help
      Print help.
  -V, --version
      Print version.
```

## Contributing

Contributions are welcome! Open a GitHub issue or pull request.

## License

This project is licensed under the [MIT license](LICENSE)
