mod cli;
mod utils;

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::process::Stdio;

use anyhow::{bail, Context, Result};
use clap::CommandFactory;
use clap::Parser;
use clap_complete::generate;
use rayon::prelude::*;
use std::path::{Path, PathBuf};

use cli::{Cli, Format};
use utils::{resolve_backend, resolve_threads, spawn_compressor, spawn_decompressor, Role};

fn main() {
    // Delegate to `run` so we can use `?` throughout, then handle the error
    // here with a clean user-facing message instead of the default Debug dump.
    if let Err(e) = run() {
        eprintln!("arc: error: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Generate shell completion script
    if let Some(shell) = cli.generate {
        generate(shell, &mut Cli::command(), "arc", &mut std::io::stdout());
        return Ok(());
    }

    // Validate that all inputs exist before starting any conversion
    for input in &cli.inputs {
        if !input.exists() {
            bail!("Input file not found: {}", input.display());
        }
        if !input.is_file() {
            bail!(
                "Input path is not a regular file: {}\n\
                arc does not handle directories.",
                input.display()
            );
        }
    }

    // Guard: if multiple inputs are given, --to is required
    if cli.inputs.len() > 1 && cli.to.is_none() {
        bail!(
            "Multiple input files given but --to <FORMAT> is missing.\n\
               Example: arc *.tar.gz --to zst"
        );
    }

    if cli.inputs.len() == 1 && cli.to.is_none() && cli.output.is_none() && !cli.stdout {
        bail!(
            "No output specified.\n\
            Provide an output path, use --to <FORMAT>, or use --stdout --format <FORMAT>."
        );
    }

    // Dispatch
    if cli.inputs.len() > 1 || cli.to.is_some() {
        run_batch(&cli)
    } else {
        run_single(&cli, &cli.inputs[0], cli.output.as_ref())
    }
}

/// Where the compressed output should be written.
enum Destination {
    Stdout,
    File(std::path::PathBuf),
}

fn run_single(cli: &Cli, input: &PathBuf, output: Option<&PathBuf>) -> Result<()> {
    // Infer input format
    let in_fmt = Format::from_path(input).with_context(|| {
        let extra = if input.extension().and_then(|e| e.to_str()) == Some("tar") {
            "\nNote: arc converts between compression formats. It cannot compress a raw .tar.\n\
             To compress it for the first time for e.g.: gzip -c FILE.tar > FILE.tar.gz"
        } else {
            ""
        };
        format!(
            "Unrecognised input format: {}\n\
             Supported extensions: .gz  .bz2  .xz  .zst  (also .tar.gz etc.){}",
            input.display(),
            extra
        )
    })?;

    // Determine output format
    // We have two mode, --stdout => write to stdout, format explict
    // and <OUTPUT> => write to file, format drawn from extension
    let (out_fmt, destination) = if cli.stdout {
        let fmt = cli.format.expect("--format required with --stdout");
        (fmt, Destination::Stdout)
    } else {
        let out_path = output.with_context(|| {
            "No output file specified.\n\
            Provide an output path or use --stdout --format <FMT> to write to stdout."
        })?;

        let fmt = Format::from_path(out_path).with_context(|| {
            format!(
                "Unrecognised output format: {}\n\
             Supported extensions: .gz  .bz2  .xz  .zst  (also .tar.gz etc.)",
                out_path.display()
            )
        })?;
        (fmt, Destination::File(out_path.to_path_buf()))
    };

    if in_fmt == out_fmt {
        bail!(
            "Input and output are both {in_fmt}, nothing to convert.\n\
             Did you mean a different output extension?"
        );
    }

    // Validate output path
    if let Destination::File(ref out_path) = destination {
        if out_path.exists() && !cli.force {
            bail!(
                "Output file already exists: {}\n\
                 Use --force to overwrite.",
                out_path.display()
            );
        }
    }

    // Resolve backends
    let dec_backend = resolve_backend(in_fmt, Role::Decompress)?;
    let enc_backend = resolve_backend(out_fmt, Role::Compress)?;

    let threads = resolve_threads(cli.threads);

    // Dry run
    if cli.dry_run {
        let dest_label = match destination {
            Destination::Stdout => "stdout".to_string(),
            Destination::File(ref p) => p.display().to_string(),
        };
        eprintln!("arc dry run - no files will be read or written");
        eprintln!();
        eprintln!("  input      : {}", input.display());
        eprintln!("  output     : {dest_label}");
        eprintln!("  conversion : {in_fmt} => {out_fmt}");
        eprintln!(
            "  decompress : {} (parallel: {})",
            dec_backend.name, dec_backend.parallel
        );
        eprintln!(
            "  compress   : {} (parallel: {})",
            enc_backend.name, enc_backend.parallel
        );
        eprintln!("  level      : {}", cli.level);
        eprintln!("  threads    : {threads}");
        eprintln!("  keep input : {}", cli.keep || cli.stdout);
        return Ok(());
    }

    // Open input file
    let in_file = File::open(input)
        .with_context(|| format!("Cannot open input file: {}", input.display()))?;

    // Spawn decompressor
    //
    //   input file  ==stdin==>  decompressor  ==stdout==>  (pipe)
    //
    let mut decompressor = spawn_decompressor(&dec_backend, in_fmt, Stdio::from(in_file))?;

    // Grab the decompressor's stdout to feed into the compressor.
    let dec_stdout = decompressor
        .stdout
        .take()
        .context("Decompressor did not open a stdout pipe")?;

    // Spawn compressor
    //
    //   (pipe)  ==stdin==>  compressor  ==stdout==>  output file
    //
    let mut compressor = spawn_compressor(
        &enc_backend,
        out_fmt,
        cli.level,
        threads,
        Stdio::from(dec_stdout),
    )?;

    // Grab the compressor's stdout and copy it to the output file.
    let mut enc_stdout = compressor
        .stdout
        .take()
        .context("Compressor did not open a stdout pipe")?;

    // Drain the pipeline
    //
    // This single copy drives the whole pipeline:
    //   in_file => decompressor => compressor => out_file
    //
    // Both child processes run concurrently; the kernel buffers the pipe
    // between them. We block here until the compressor closes its stdout.
    match destination {
        Destination::Stdout => {
            let stdout = io::stdout();
            let mut out = BufWriter::new(stdout.lock());
            io::copy(&mut enc_stdout, &mut out).context("I/O error while writing to stdout")?;
            out.flush().context("Failed to flush stdout")?;
        }
        Destination::File(ref out_path) => {
            let out_file = File::create(out_path)
                .with_context(|| format!("Cannot create output file: {}", out_path.display()))?;
            let mut out = BufWriter::new(out_file);
            io::copy(&mut enc_stdout, &mut out).context("I/O error while writing output file")?;
        }
    }

    // Wait for child processes
    //
    // Wait for decompressor first: if it failed, the compressor likely also
    // failed, so the decompressor's exit code is more informative.
    let dec_status = decompressor
        .wait()
        .with_context(|| format!("Failed waiting for decompressor '{}'", dec_backend.name))?;

    if !dec_status.success() {
        // Best-effort: remove the incomplete output file before bailing.
        if let Destination::File(ref out_path) = destination {
            let _ = std::fs::remove_file(out_path);
        }
        bail!(
            "Decompressor '{}' failed (exit code {}).\n\
             Check stderr above for details.",
            dec_backend.name,
            dec_status.code().unwrap_or(-1),
        );
    }

    let enc_status = compressor
        .wait()
        .with_context(|| format!("Failed waiting for compressor '{}'", enc_backend.name))?;

    if !enc_status.success() {
        if let Destination::File(ref out_path) = destination {
            let _ = std::fs::remove_file(out_path);
        }
        bail!(
            "Compressor '{}' failed (exit code {}).\n\
             Check stderr above for details.",
            enc_backend.name,
            enc_status.code().unwrap_or(-1),
        );
    }

    // Cleanup
    if !cli.keep {
        if let Err(e) = std::fs::remove_file(input) {
            eprintln!("arc: warning: could not remove {}: {e}", input.display());
        }
    }

    Ok(())
}

fn run_batch(cli: &Cli) -> Result<()> {
    let out_fmt = cli.to.expect("--to guaranteed present in batch mode");

    // Create --outdir if specified and not present
    if let Some(ref dir) = cli.outdir {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("Cannot create output directory: {}", dir.display()))?;
    }

    // Collect results: run all conversions in parallel via rayon.
    // Each conversion is independent, they share no state.
    let results: Vec<(&PathBuf, Result<()>)> = cli
        .inputs
        .par_iter()
        .map(|input| {
            let output = output_path_for(input, out_fmt, cli.outdir.as_ref());
            (input, run_single(cli, input, Some(&output)))
        })
        .collect();

    // Report: print all errors, then fail if any conversion failed.
    let mut failed = 0usize;
    for (input, result) in &results {
        if let Err(e) = result {
            eprintln!("arc: error: {}: {e:#}", input.display());
            failed += 1;
        }
    }

    if failed > 0 {
        bail!("{failed} of {} conversion(s) failed", cli.inputs.len());
    }

    Ok(())
}

/// Derive the output path for a batch input file.
///
/// Replaces the compression extension with the target format's extension.
/// If --outdir is given, places the output file there instead of alongside
/// the input.
///
/// linux.tar.gz  --to zst  =>  linux.tar.zst
/// linux.tar.gz  --to zst --outdir / =>  /linux.tar.zst
fn output_path_for(input: &Path, fmt: Format, outdir: Option<&PathBuf>) -> PathBuf {
    // Strip the compression extension, append the new one.
    let stem = input
        .file_stem() // "linux.tar" from "linux.tar.gz"
        .unwrap_or(input.as_os_str());
    let new_name = format!("{}.{}", stem.to_string_lossy(), fmt.ext());

    match outdir {
        Some(dir) => dir.join(new_name),
        None => input.with_file_name(new_name),
    }
}
