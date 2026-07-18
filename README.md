# arc

[![CI](https://github.com/Ebedthan/arc/actions/workflows/ci.yml/badge.svg)](https://github.com/Ebedthan/arc/actions/workflows/ci.yml)
[![CD](https://github.com/Ebedthan/arc/actions/workflows/cd.yml/badge.svg)](https://github.com/Ebedthan/arc/actions/workflows/cd.yml)
[![codecov](https://codecov.io/gh/Ebedthan/arc/branch/main/graph/badge.svg)](https://codecov.io/gh/Ebedthan/arc)

Convert between compression formats without a temporary file.
 
```sh
arc file.tar.gz --output file.tar.zst
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

### Shell completions
 
arc can generate completion scripts for your shell directly:
 
```sh
# Bash - add to ~/.bashrc
arc --generate bash >> ~/.bashrc
 
# Zsh - write to a directory on your $fpath
arc --generate zsh > ~/.zfunc/_arc
autoload -Uz compinit && compinit
 
# Fish
arc --generate fish > ~/.config/fish/completions/arc.fish
 
# Elvish
arc --generate elvish > ~/.config/elvish/lib/arc.elv
echo "use arc" >> ~/.config/elvish/rc.elv
 
# PowerShell - add to your $PROFILE
arc --generate powershell >> $PROFILE
```
 
### Options
 
| Flag | Default | Description |
|------|---------|-------------|
| `-o`, `--output <FILE>` | — | Output file for single-file mode (format inferred from extension) |
| `--to <FORMAT>` | — | Target format for batch mode: `gz`, `bz2`, `xz`, `zst` |
| `--outdir <DIR>` | — | Directory to write output files into (batch mode, requires `--to`) |
| `-c`, `--stdout` | off | Write output to stdout (requires `--format`) |
| `-F`, `--format <FORMAT>` | — | Target format for `--stdout` mode |
| `-l`, `--level <N>` | `6` | Compression level, 1 (fastest) to 9 (smallest) |
| `-j`, `--threads <N>` | `1` | Threads to use; `0` = all available cores |
| `-k`, `--keep` | off | Keep input files after conversion |
| `-f`, `--force` | off | Overwrite output files if they already exist |
| `--dry-run` | off | Show what would happen without writing anything |
| `--generate <SHELL>` | — | Print shell completion script and exit |
 
### Examples
 
**Single file conversion**
```sh
# Basic conversion
arc file.tar.gz --output file.tar.xz
 
# Use all cores, maximum compression
arc data.gz --output data.zst --threads 0 --level 9
 
# Fast recompression, keep the original
arc logs.bz2 --output logs.gz --level 1 --keep
 
# Overwrite existing output
arc archive.xz --output archive.zst --force
```
 
**Batch conversion**
```sh
# Convert all gz archives in the current directory to zst
arc *.tar.gz --to zst
 
# Convert to a different directory
arc *.tar.bz2 --to xz --outdir /backup/xz
 
# Dry run first to see what would happen
arc *.tar.gz --to zst --dry-run
 
# Batch with all cores, keep originals
arc *.gz --to zst --threads 0 --keep
```
 
**Stdout mode**
```sh
# Pipe to a remote host
arc archive.tar.gz --stdout --format zst | ssh host "cat > archive.tar.zst"
 
# Count bytes without writing a file
arc big.tar.bz2 --stdout --format xz | wc -c
```
 
**Shell completions**
```sh
arc --generate bash >> ~/.bashrc
```
 
**Dry run**
```sh
arc linux.tar.gz --output linux.tar.zst --threads 0 --level 9 --dry-run
```
```
arc dry run - no files will be read or written
 
  input      : linux.tar.gz
  output     : linux.tar.zst
  conversion : gzip => zstd
  decompress : pigz (parallel: true)
  compress   : zstd (parallel: true)
  level      : 9
  threads    : 12
  keep input : false
```

# arc benchmark results

**Date:** 2026-07-15 13:04
**Host:** Linux 6.17.0-35-generic x86_64
**Cores:** 8 logical cores
**arc:** arc 0.1.0
**hyperfine:** hyperfine 1.20.0
**Backends:** pigz pigz 2.8, pbzip2 Parallel BZIP2 v1.1.13 [Dec 18, 2015], xz xz (XZ Utils) 5.4.5, zstd *** Zstandard CLI (64-bit) v1.5.5, by Yann Collet ***

Warmup runs: 3 | Timed runs: 10
Temporary output directory: `/tmp/arc_bench`


## Input: `linux.tar.gz` — Linux kernel tarball (~1.4 GB uncompressed, source code) (227M compressed)

### 1. Format comparison — single-threaded, level 6

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `gz  → bz2` | 37.161 ± 1.070 | 35.057 | 39.076 | 1.00 |
| `gz  → xz` | 461.899 ± 2.465 | 458.522 | 466.361 | 12.43 ± 0.36 |
| `gz  → zst` | 52.506 ± 0.568 | 51.883 | 53.378 | 1.41 ± 0.04 |

### 2. Parallelism scaling — gz → zst, level 6

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `j=1` | 53.249 ± 0.595 | 52.457 | 54.248 | 2.18 ± 0.10 |
| `j=2` | 33.058 ± 0.592 | 32.176 | 33.778 | 1.35 ± 0.07 |
| `j=4` | 24.407 ± 1.135 | 22.926 | 25.870 | 1.00 |
| `j=all (8)` | 26.493 ± 0.521 | 25.803 | 27.273 | 1.09 ± 0.05 |

### 3. Level comparison — gz → zst, all cores

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `level 1 (fastest)` | 5.607 ± 0.248 | 5.360 | 6.136 | 1.00 |
| `level 3` | 9.823 ± 1.374 | 7.994 | 12.547 | 1.75 ± 0.26 |
| `level 6 (default)` | 26.752 ± 1.708 | 24.542 | 29.771 | 4.77 ± 0.37 |
| `level 9 (smallest)` | 330.077 ± 4.521 | 322.394 | 336.940 | 58.87 ± 2.73 |

### 4. Output sizes — level 6, single-threaded

| Format | Compressed size | Ratio vs input |
|--------|----------------|----------------|
| .gz (input) | 227M | 1.00x (baseline) |
| .bz2 | 175M | 0.77x |
| .xz | 138M | 0.61x |
| .zst | 165M | 0.73x |


## Input: `silesia.tar.gz` — Silesia corpus (~211 MB uncompressed, mixed content) (66M compressed)

### 1. Format comparison — single-threaded, level 6

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `gz  → bz2` | 7.066 ± 0.424 | 6.593 | 7.719 | 1.00 |
| `gz  → xz` | 95.094 ± 0.548 | 94.302 | 96.333 | 13.46 ± 0.81 |
| `gz  → zst` | 12.612 ± 0.105 | 12.457 | 12.749 | 1.78 ± 0.11 |

### 2. Parallelism scaling — gz → zst, level 6

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `j=1` | 12.650 ± 0.279 | 12.260 | 13.148 | 1.89 ± 0.14 |
| `j=2` | 8.357 ± 0.194 | 8.092 | 8.793 | 1.25 ± 0.09 |
| `j=4` | 6.798 ± 0.274 | 6.325 | 7.198 | 1.01 ± 0.08 |
| `j=all (8)` | 6.698 ± 0.485 | 5.821 | 7.136 | 1.00 |

### 3. Level comparison — gz → zst, all cores

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `level 1 (fastest)` | 1.267 ± 0.099 | 1.062 | 1.349 | 1.00 |
| `level 3` | 2.424 ± 0.026 | 2.372 | 2.456 | 1.91 ± 0.15 |
| `level 6 (default)` | 6.836 ± 0.472 | 5.897 | 7.207 | 5.39 ± 0.56 |
| `level 9 (smallest)` | 52.784 ± 1.438 | 49.592 | 54.624 | 41.65 ± 3.44 |

### 4. Output sizes — level 6, single-threaded

| Format | Compressed size | Ratio vs input |
|--------|----------------|----------------|
| .gz (input) | 66M | 1.00x (baseline) |
| .bz2 | 54M | 0.82x |
| .xz | 47M | 0.72x |
| .zst | 56M | 0.85x |


## Input: `random.bin.gz` — Random binary data (512 MB, incompressible) (513M compressed)

### 1. Format comparison — single-threaded, level 6

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `gz  → bz2` | 27.275 ± 0.973 | 24.921 | 28.226 | 7.83 ± 0.29 |
| `gz  → xz` | 250.181 ± 4.839 | 244.857 | 256.784 | 71.81 ± 1.50 |
| `gz  → zst` | 3.484 ± 0.028 | 3.418 | 3.510 | 1.00 |

### 2. Parallelism scaling — gz → zst, level 6

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `j=1` | 3.486 ± 0.056 | 3.335 | 3.521 | 1.99 ± 0.06 |
| `j=2` | 2.402 ± 0.019 | 2.370 | 2.431 | 1.37 ± 0.04 |
| `j=4` | 1.835 ± 0.047 | 1.771 | 1.907 | 1.05 ± 0.04 |
| `j=all (8)` | 1.754 ± 0.043 | 1.681 | 1.814 | 1.00 |

### 3. Level comparison — gz → zst, all cores

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `level 1 (fastest)` | 1.227 ± 0.525 | 0.940 | 2.263 | 1.00 |
| `level 3` | 1.288 ± 0.379 | 0.899 | 2.078 | 1.05 ± 0.55 |
| `level 6 (default)` | 1.879 ± 0.148 | 1.730 | 2.101 | 1.53 ± 0.67 |
| `level 9 (smallest)` | 49.140 ± 0.264 | 48.824 | 49.606 | 40.06 ± 17.15 |

### 4. Output sizes — level 6, single-threaded

| Format | Compressed size | Ratio vs input |
|--------|----------------|----------------|
| .gz (input) | 513M | 1.00x (baseline) |
| .bz2 | 515M | 1.01x |
| .xz | 513M | 1.00x |
| .zst | 513M | 1.00x |


## Batch conversion — all three input files

### 5. Batch mode (--to) vs sequential single-file calls

Converts linux.tar.gz + silesia.tar.gz + random.bin.gz in one invocation
vs three sequential arc calls. Measures rayon's cross-file parallelism.

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `sequential (3x arc)` | 39.026 ± 1.049 | 37.083 | 40.127 | 1.00 |
| `batch (arc --to zst)` | 39.687 ± 0.089 | 39.539 | 39.799 | 1.02 ± 0.03 |

### 6. Batch parallelism scaling — all three files, gz → zst

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `j=1` | 65.337 ± 0.523 | 64.651 | 66.150 | 1.65 ± 0.01 |
| `j=4` | 39.531 ± 0.124 | 39.368 | 39.777 | 1.00 |
| `j=all (8)` | 39.772 ± 0.103 | 39.566 | 39.955 | 1.01 ± 0.00 |


## Methodology

- Each timed command was run **10 times** after **3 warmup runs** to reduce
  cold-cache and scheduler noise.
- hyperfine reports the **mean ± standard deviation** across timed runs.
- All conversions use `-k` (keep input) and `-f` (force overwrite) so the
  input file survives the full benchmark and hyperfine can repeat each run.
- Output files are written to a temporary directory; results reflect wall-clock
  time including process spawn overhead, which is negligible for large files.
- "Ratio vs input" compares the output byte count to the **original .gz input**
  size, not the uncompressed size, so values above 1.0x mean the target
  format is larger than gzip at level 6.
- Suites 5 and 6 measure batch mode: arc dispatches file conversions in parallel
  via rayon, so total wall time for N files can be less than N × single-file time
  when enough cores are available.

## Notes
 
**arc removes input files on success** unless `--keep` is passed. This mirrors
the behaviour of `gzip` and `xz`. If a conversion fails for any reason, the
input file is left untouched and any partial output file is removed.
 
**In batch mode, all errors are reported before exiting.** arc does not stop at
the first failure - every file is attempted and all failures are printed
together so you can fix them in one pass.
 
**Compression levels are normalised across formats.** Level 1 always means
"fastest, largest output" and level 9 always means "slowest, smallest output",
regardless of the underlying tool. For zstd, arc maps its 1-9 scale onto
zstd's native 1-19 range.
 
**`--stdout` implies `--keep`.** When writing to stdout there is no persistent
output file, so arc never removes the input in that mode.
 
**zip / tar conversion is out of scope.** `zip` and `tar` are structurally
incompatible archive formats - converting between them requires fully extracting
and repacking all files, which is a different class of operation. arc is
intentionally limited to recompression of a fixed archive stream.
 
## Dependencies
 
```toml
[dependencies]
anyhow        = "1"
clap          = { version = "4.6", features = ["derive"] }
clap_complete = "4.5"
rayon         = "1"
which         = "7"
```

## License

MIT
