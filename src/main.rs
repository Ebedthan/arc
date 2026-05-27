mod cli;
mod utils;

use std::fs::File;
use std::io::{self, BufWriter};
use std::process::Stdio;

use anyhow::{bail, Context, Result};
use clap::Parser;

use cli::{Cli, Format};
use utils::{
    resolve_backend, resolve_threads, setup_logging, spawn_compressor, spawn_decompressor, Role,
};

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

    // Logging
    setup_logging(cli.verbose)?;

    // Validate input path
    if !cli.input.exists() {
        bail!("Input file not found: {}", cli.input.display());
    }
    if !cli.input.is_file() {
        bail!(
            "Input path is not a regular file: {}\n\
             arc does not handle directories.",
            cli.input.display()
        );
    }

    // Infer formats
    let in_fmt = Format::from_path(&cli.input).with_context(|| {
        format!(
            "Unrecognised input format: {}\n\
             Supported extensions: .gz  .bz2  .xz  .zst  (also .tar.gz etc.)",
            cli.input.display()
        )
    })?;

    let out_fmt = Format::from_path(&cli.output).with_context(|| {
        format!(
            "Unrecognised output format: {}\n\
             Supported extensions: .gz  .bz2  .xz  .zst  (also .tar.gz etc.)",
            cli.output.display()
        )
    })?;

    if in_fmt == out_fmt {
        bail!(
            "Input and output are both {in_fmt}, nothing to convert.\n\
             Did you mean a different output extension?"
        );
    }

    // Validate output path
    if cli.output.exists() && !cli.force {
        bail!(
            "Output file already exists: {}\n\
             Use --force to overwrite.",
            cli.output.display()
        );
    }

    // Resolve backends
    let dec_backend = resolve_backend(in_fmt, Role::Decompress)?;
    let enc_backend = resolve_backend(out_fmt, Role::Compress)?;

    let threads = resolve_threads(cli.threads);

    log::info!(
        "Converting {} => {}  (decompress: {}, compress: {}, level: {}, threads: {})",
        cli.input.display(),
        cli.output.display(),
        dec_backend.name,
        enc_backend.name,
        cli.level,
        threads,
    );

    // Open input file
    let in_file = File::open(&cli.input)
        .with_context(|| format!("Cannot open input file: {}", cli.input.display()))?;

    // Open output file
    let out_file = File::create(&cli.output)
        .with_context(|| format!("Cannot create output file: {}", cli.output.display()))?;
    let out_writer = BufWriter::new(out_file);

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
    let bytes_written = io::copy(&mut enc_stdout, &mut { out_writer })
        .context("I/O error while draining compressor output")?;

    log::debug!(
        "Wrote {bytes_written} compressed bytes to {}",
        cli.output.display()
    );

    // Wait for child processes
    //
    // Wait for decompressor first: if it failed, the compressor likely also
    // failed, so the decompressor's exit code is more informative.
    let dec_status = decompressor
        .wait()
        .with_context(|| format!("Failed waiting for decompressor '{}'", dec_backend.name))?;

    if !dec_status.success() {
        // Best-effort: remove the incomplete output file before bailing.
        let _ = std::fs::remove_file(&cli.output);
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
        let _ = std::fs::remove_file(&cli.output);
        bail!(
            "Compressor '{}' failed (exit code {}).\n\
             Check stderr above for details.",
            enc_backend.name,
            enc_status.code().unwrap_or(-1),
        );
    }

    log::info!("Done: {}", cli.output.display());

    // Cleanup
    if !cli.keep {
        if let Err(e) = std::fs::remove_file(&cli.input) {
            log::warn!(
                "Conversion succeeded but could not remove input file {}: {e}\n\
                 You may want to delete it manually.",
                cli.input.display()
            );
        }
    }

    Ok(())
}
