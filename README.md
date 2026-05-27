# arc

[![CI](https://github.com/Ebedthan/arc/actions/workflows/ci.yml/badge.svg)](https://github.com/Ebedthan/arc/actions/workflows/ci.yml)
[![CD](https://github.com/Ebedthan/arc/actions/workflows/cd.yml/badge.svg)](https://github.com/Ebedthan/arc/actions/workflows/cd.yml)
[![codecov](https://codecov.io/gh/Ebedthan/arc/branch/main/graph/badge.svg)](https://codecov.io/gh/Ebedthan/arc)

Convert between compression formats without a temporary file.

```
arc archive.tar.gz archive.tar.zst
```

Instead of remembering the right flags for each tool and writing the pipe
yourself, arc figures out the formats from the file extensions, picks the
fastest available backend, and streams the conversion in one pass.

## Why not just use a pipe?
 
You can. This works perfectly:
 
```sh
gzip -dc archive.tar.gz | xz -c > archive.tar.xz
```
 
So does this:
 
```sh
pigz -dc archive.tar.gz | zstd -T0 -9 -c > archive.tar.zst
```
 
And this, if you remembered that pbzip2 takes `-p` not `-T`, that zstd levels
go to 19 not 9, that gzip's decompression flag is `-d` not `-D`, and that the
output redirection goes *after* the last pipe segment and not before it:
 
```sh
pbzip2 -dc -p8 archive.tar.bz2 | zstd -T8 -14 -c > archive.tar.zst
```
 
arc does not make compression faster. It does not invent new formats. It just
remembers all of that for you:
 
```sh
arc archive.tar.bz2 archive.tar.zst -j8 -l9
```
 
That's it. Formats from the extensions. Parallel backends picked automatically.
Level scale normalised. Input file removed on success, like every other
compression tool you already use.
 
The pipe is always there if you want it. arc is for the days you don't :).

## How it works

arc spawns two processes and pipes them together:

```
input file  ==>  decompressor  ==>  compressor  ==>  output file
```

No intermediate file is written to disk. The kernel buffers the pipe between
the two processes, so both run concurrently. For large files this is
meaningfully faster than decompress-then-compress.


## Supported formats

| Extension       | Format  | Parallel backend | Fallback  |
|-----------------|---------|-----------------|-----------|
| `.gz` / `.gzip` | gzip    | `pigz`          | `gzip`    |
| `.bz2` / `.bzip2` | bzip2 | `pbzip2`        | `bzip2`   |
| `.xz` / `.lzma` | xz      | `xz -T`         | —         |
| `.zst` / `.zstd` | zstd   | `zstd -T`       | —         |

Compound extensions like `.tar.gz`, `.tar.bz2`, `.tar.xz`, `.tar.zst` are
fully supported, the tar layer passes through untouched.


## Installation

### From source

```sh
git clone https://github.com/Ebedthan/arc
cd arc
cargo build --release
cp target/release/arc ~/.local/bin/
```

Requires Rust 1.70 or later. Install via [rustup](https://rustup.rs) if needed.

### Backend binaries

arc delegates compression to external binaries. The single-threaded fallbacks
(`gzip`, `bzip2`, `xz`, `zstd`) are pre-installed on most systems. The
parallel backends are optional but recommended for large files:

**Debian / Ubuntu**
```sh
sudo apt install pigz pbzip2
```

**Fedora / RHEL**
```sh
sudo dnf install pigz pbzip2
```

**macOS**
```sh
brew install pigz pbzip2
```

`xz` and `zstd` support multithreading natively (via `-T`), no separate
parallel binary needed for those.

If a required backend is missing, arc will tell you exactly what to install
and exit cleanly.

---

## Usage

```
arc <INPUT> <OUTPUT> [OPTIONS]
```

### Options

| Flag | Default | Description |
|------|---------|-------------|
| `-l`, `--level <N>` | `6` | Compression level, 1 (fastest) to 9 (smallest) |
| `-j`, `--threads <N>` | `1` | Threads to use; `0` = all available cores |
| `-k`, `--keep` | off | Keep the input file after conversion |
| `-f`, `--force` | off | Overwrite the output file if it already exists |
| `-v`, `--verbose` | off | Increase log verbosity (repeat for more: `-vvv`) |

### Examples

```sh
# Basic conversion
arc backup.tar.gz backup.tar.xz

# Use all cores, maximum compression
arc data.gz data.zst --threads 0 --level 9

# Fast recompression, keep the original
arc logs.bz2 logs.gz --level 1 --keep

# Overwrite existing output
arc archive.xz archive.zst --force
```

---

## Notes

**arc removes the input file on success** unless `--keep` is passed. This
mirrors the behaviour of `gzip` and `xz`. If conversion fails for any reason,
the input file is left untouched and any partial output file is removed.

**Compression levels are normalised across formats.** Level 1 always means
"fastest, largest output" and level 9 always means "slowest, smallest output",
regardless of the underlying tool. For zstd, arc maps its 1-9 scale onto
zstd's native 1-19 range.

**zip / tar conversion is out of scope.** `zip` and `tar` are structurally
incompatible archive formats, converting between them requires fully
extracting and repacking all files, which is a different class of operation.
arc is intentionally limited to recompression of a fixed archive stream.


## Dependencies

```toml
[dependencies]
anyhow  = "1"
clap    = { version = "4.6", features = ["derive"] }
fern    = "0.7"
log     = "0.4"
which   = "7"
```

## License

MIT
