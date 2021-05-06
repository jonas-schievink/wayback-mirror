# Wayback Machine downloader

[![crates.io](https://img.shields.io/crates/v/wayback-mirror.svg)](https://crates.io/crates/wayback-mirror)
![CI](https://github.com/jonas-schievink/wayback-mirror/workflows/CI/badge.svg)

This is a small command-line utility I wrote to help with browsing archived websites from the [Wayback Machine], which can sometimes be pretty slow.

Please refer to the [changelog](CHANGELOG.md) to see what changed in the last
releases.

[Wayback Machine]: http://web.archive.org/

## Usage

Install it via:

```shell
$ cargo install wayback-mirror
```

Usage:

```shell
$ wayback-mirror --out-dir <directory> <url>
```
