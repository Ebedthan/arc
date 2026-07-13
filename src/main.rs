mod cli;
mod utils;

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::process::Stdio;

use anyhow::{bail, Context, Result};
use clap::CommandFactory;
use clap::Parser;
use clap_complete::generate;

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

    // Infer input format
    let in_fmt = Format::from_path(&cli.input).with_context(|| {
        let extra = if cli.input.extension().and_then(|e| e.to_str()) == Some("tar") {
            "\nNote: arc converts between compression formats. It cannot compress a raw .tar.\n\
             To compress it for the first time for e.g.: gzip -c FILE.tar > FILE.tar.gz"
        } else {
            ""
        };
        format!(
            "Unrecognised input format: {}\n\
             Supported extensions: .gz  .bz2  .xz  .zst  (also .tar.gz etc.){}",
            cli.input.display(),
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
        let out_path = cli.output.as_ref().with_context(|| {
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
        (fmt, Destination::File(out_path.clone()))
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

    // Open input file
    let in_file = File::open(&cli.input)
        .with_context(|| format!("Cannot open input file: {}", cli.input.display()))?;

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
        if let Err(e) = std::fs::remove_file(&cli.input) {
            eprintln!(
                "arc: warning: could not remove {}: {e}",
                cli.input.display()
            );
        }
    }

    Ok(())
}

/// Where the compressed output should be written.
enum Destination {
    Stdout,
    File(std::path::PathBuf),
}
