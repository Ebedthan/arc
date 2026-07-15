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
  gzip → zstd
  decompress : pigz (parallel: true)
  compress   : zstd (parallel: true)
  level      : 9
  threads    : 12
  keep input : false
```
 

## arc benchmark

This a simple benchmark to help the user into choosing the right compression format and level for their use case. It is not a comprehensive benchmark, but it should give a good indication of the performance of each compression format.

**Date:** 2026-05-27 17:35  
**Host:** Linux 6.17.0-29-generic x86_64  
**Cores:** 8 logical cores  
**arc:** arc 0.1.0  
**hyperfine:** hyperfine 1.20.0  
**Backends:** pigz pigz 2.8, pbzip2 Parallel BZIP2 v1.1.13 [Dec 18, 2015], xz xz (XZ Utils) 5.4.5, zstd **Zstandard CLI (64-bit) v1.5.5, by Yann Collet**

Warmup runs: 1 | Timed runs: 5


### Input: `linux.tar.gz` - Linux kernel tarball (~1.4 GB uncompressed, source code) (227M compressed)

#### 1. Format comparison (single-threaded, level 6)

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `gz  => bz2` | 44.524 ± 2.021 | 41.166 | 46.628 | 1.00 |
| `gz  => xz` | 741.807 ± 1.843 | 739.385 | 744.021 | 16.66 ± 0.76 |
| `gz  => zst` | 72.989 ± 0.196 | 72.784 | 73.275 | 1.64 ± 0.07 |

#### 2. Parallelism scaling (gz => zst, level 6)

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `j=1` | 62.430 ± 9.931 | 54.024 | 73.315 | 2.46 ± 0.40 |
| `j=2` | 35.163 ± 0.489 | 34.510 | 35.737 | 1.38 ± 0.04 |
| `j=4` | 25.419 ± 0.648 | 24.863 | 26.475 | 1.00 |
| `j=all (8)` | 25.737 ± 0.547 | 24.996 | 26.293 | 1.01 ± 0.03 |

#### 3. Level comparison (gz => zst, all cores)

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `level 1 (fastest)` | 5.381 ± 0.371 | 5.039 | 5.966 | 1.00 |
| `level 3` | 8.851 ± 0.547 | 8.327 | 9.631 | 1.64 ± 0.15 |
| `level 6 (default)` | 26.291 ± 0.789 | 25.082 | 27.124 | 4.89 ± 0.37 |
| `level 9 (smallest)` | 290.657 ± 2.526 | 287.223 | 292.639 | 54.01 ± 3.75 |

#### 4. Output sizes (level 6, single-threaded)

| Format | Compressed size | Ratio vs input |
|--------|----------------|----------------|
| .gz (input) | 227M | 1.00x (baseline) |
| .bz2 | 175M | 0.77x |
| .xz | 138M | 0.61x |
| .zst | 165M | 0.73x |


### Input: `silesia.tar.gz` (Silesia corpus (~211 MB uncompressed, mixed content) (66M compressed))

#### 1. Format comparison (single-threaded, level 6)

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `gz  => bz2` | 5.788 ± 0.449 | 5.443 | 6.291 | 1.00 |
| `gz  => xz` | 99.729 ± 0.511 | 99.021 | 100.341 | 17.23 ± 1.34 |
| `gz  => zst` | 11.662 ± 0.244 | 11.239 | 11.873 | 2.01 ± 0.16 |

#### 2. Parallelism scaling (gz => zst, level 6)

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `j=1` | 11.890 ± 0.318 | 11.605 | 12.285 | 2.15 ± 0.16 |
| `j=2` | 7.296 ± 0.392 | 6.676 | 7.695 | 1.32 ± 0.12 |
| `j=4` | 5.536 ± 0.397 | 5.085 | 5.914 | 1.00 |
| `j=all (8)` | 5.829 ± 0.108 | 5.642 | 5.896 | 1.05 ± 0.08 |

#### 3. Level comparison (gz => zst, all cores)

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `level 1 (fastest)` | 1.165 ± 0.130 | 0.993 | 1.334 | 1.00 |
| `level 3` | 1.903 ± 0.208 | 1.640 | 2.175 | 1.63 ± 0.25 |
| `level 6 (default)` | 5.976 ± 0.226 | 5.645 | 6.247 | 5.13 ± 0.60 |
| `level 9 (smallest)` | 46.159 ± 1.584 | 43.612 | 47.818 | 39.63 ± 4.61 |

#### 4. Output sizes (level 6, single-threaded)

| Format | Compressed size | Ratio vs input |
|--------|----------------|----------------|
| .gz (input) | 66M | 1.00x (baseline) |
| .bz2 | 54M | 0.82x |
| .xz | 47M | 0.72x |
| .zst | 56M | 0.85x |


### Input: `random.bin.gz` (Random binary data (512 MB, incompressible) (513M compressed))

#### 1. Format comparison (single-threaded, level 6)

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `gz  => bz2` | 22.304 ± 0.514 | 21.760 | 23.146 | 6.14 ± 0.17 |
| `gz  => xz` | 264.834 ± 0.602 | 263.864 | 265.515 | 72.94 ± 1.19 |
| `gz  => zst` | 3.631 ± 0.059 | 3.558 | 3.715 | 1.00 |

#### 2. Parallelism scaling (gz => zst, level 6)

| Command | Mean [s] | Min [s] | Max [s] | Relative |
|:---|---:|---:|---:|---:|
| `j=1` | 3.543 ± 0.133 | 3.322 | 3.661 | 1.94 ± 0.10 |
| `j=2` | 2.443 ± 0.015 | 2.419 | 2.455 | 1.34 ± 0.05 |
| `j=4` | 1.831 ± 0.101 | 1.664 | 1.933 | 1.01 ± 0.06 |
| `j=all (8)` | 1.822 ± 0.061 | 1.753 | 1.910 | 1.00 |

#### 3. Level comparison (gz => zst, all cores)

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `level 1 (fastest)` | 897.9 ± 27.6 | 869.8 | 933.0 | 1.00 |
| `level 3` | 989.0 ± 14.5 | 972.0 | 1011.5 | 1.10 ± 0.04 |
| `level 6 (default)` | 1729.0 ± 77.0 | 1592.2 | 1776.1 | 1.93 ± 0.10 |
| `level 9 (smallest)` | 49363.8 ± 250.9 | 49052.1 | 49717.9 | 54.98 ± 1.71 |

#### 4. Output sizes (level 6, single-threaded)

| Format | Compressed size | Ratio vs input |
|--------|----------------|----------------|
| .gz (input) | 513M | 1.00x (baseline) |
| .bz2 | 515M | 1.01x |
| .xz | 513M | 1.00x |
| .zst | 513M | 1.00x |


### Methodology

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
