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
$ curl -LO https://github.com/attilarepka/expurgator/releases/download/0.1.5/expurgator_0.1.5_amd64.deb
$ sudo dpkg -i expurgator_0.1.5_amd64.deb
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
Usage: expurgator [OPTIONS] --index <INDEX> <INPUT> <FILTER>

Arguments:
  <INPUT>   Input archive file
  <FILTER>  CSV file specifying which files or paths should be removed from the input archive

Options:
  -i, --index <INDEX>              Index of the CSV column specifying which files or paths should be removed from the input archive
  -w, --with-headers               Specify this flag if the CSV contains a header record [default: false]
  -o, --output <OUTPUT>            Output file [default: --input]
  -c, --compression <COMPRESSION>  Compression level [default: 6]
  -h, --help                       Print help
  -V, --version                    Print version
```

## Contributing

Contributions are welcome! Open a GitHub issue or pull request.

## License

This project is licensed under the [MIT license](LICENSE)
