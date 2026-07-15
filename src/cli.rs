use std::path::Path;
use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use clap_complete::Shell;

#[derive(Parser, Debug)]
#[command(
    name = "arc",
    version,
    author,
    about,
    long_about = None,
    after_help = "Tip: `arc -h` for a short overview, `arc --help` for full details."
)]
pub struct Cli {
    /// One or more input files. Format are detected from the file extension.
    ///
    /// Supported formats: .gz, .bz2, .xz, .zst
    /// Compound extensions are also supported: .tar.gz, .tar.bz2, .tar.xz, .tar.zst
    /// When multiple files are given, --to is required.
    #[arg(required_unless_present = "generate")]
    pub inputs: Vec<PathBuf>,

    /// Output file. Only valid when a single input is provided.
    /// Target format is inferred from the extension.
    /// When multiple inputs are given, use --to and optionally --outdir instead.
    ///
    /// Supported output formats: .gz, .bz2, .xz, .zst
    /// Required unless --to, --stdout, or --generate is used.
    /// Example: arc file.tar.gz --output file.tar.zst
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Target format for batch conversion (required when multiple inputs are given).
    ///
    /// arc will rewrite each input file's extension to the target format.
    /// Example: arc *.tar.gz --to zst => foo.tar.zst, bar.tar.zst
    #[arg(long, value_enum, value_name = "FORMAT", conflicts_with = "format")]
    pub to: Option<Format>,

    /// Directory to write output files into (batch mode only).
    ///
    /// Defaults to the same directory as each input file.
    /// Created automatically if it does not exist.
    #[arg(long, value_name = "DIR", requires = "to")]
    pub outdir: Option<PathBuf>,

    /// Write compressed output to stdout instead of a file.
    ///
    /// Requires --format to specify the target compression format,
    /// Since there is no output filename to infer it from.
    /// Implies --keep (the input file is never removed in stdout mode).
    #[arg(short = 'c', long, requires = "format")]
    pub stdout: bool,

    /// Target compression format for --stdout mode.
    ///
    /// Ignored when an output file is provided.
    #[arg(short = 'F', long, value_enum, value_name = "FORMAT")]
    pub format: Option<Format>,

    /// Compression level (1 = fastest, 9 = smallest output).
    ///
    /// The meaning of each level varies slightly by format, but the
    /// scale is normalized: 1 is always the lightest, 9 the heaviest.
    #[arg(
        short,
        long,
        default_value = "6",
        value_name = "N",
        value_parser = clap::value_parser!(u8).range(1..=9)
    )]
    pub level: u8,

    /// Number of threads to use for compression.
    ///
    /// 0 means use all available logical cores. Not all formats
    /// support multithreading (gz requires pigz, bz2 requires pbzip2;
    /// xz and zst have native support). Falls back to single-threaded
    /// if the parallel backend is unavailable.
    #[arg(short = 'j', long, default_value = "1", value_name = "N")]
    pub threads: u32,

    /// Keep the input file after conversion.
    ///
    /// By default, arc removes the input file on success,
    /// mimicking the behaviour of gzip/xz.
    #[arg(short, long)]
    pub keep: bool,

    /// Overwrite the output file if it already exists.
    #[arg(short, long)]
    pub force: bool,

    /// Generate shell completion script and print to stdout
    #[arg(long, value_name = "SHELL", value_enum)]
    pub generate: Option<Shell>,

    /// Show what would happen without writing any output.
    ///
    /// arc will resolve formats, select backends, and validate all paths.
    /// then print a summary and exit without touching any file.
    /// Usefull before committing to a slow conversion of a large file.
    #[arg(long)]
    pub dry_run: bool,
}

/// Compression formats supported by arc.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Gz,
    Bz2,
    Xz,
    Zst,
}

impl Format {
    /// Infer the format from a file path's extension(s).
    ///
    /// Handles both simple extensions (`.gz`) and compound ones (`.tar.gz`).
    /// Returns `None` if the extension is absent or unrecognised.
    pub fn from_path(path: &Path) -> Option<Self> {
        // Walk extensions from the right so `.tar.gz` => `gz`.
        let ext = path.extension()?.to_str()?;
        Self::from_ext(ext)
    }

    pub fn from_ext(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "gz" | "gzip" => Some(Self::Gz),
            "bz2" | "bzip2" => Some(Self::Bz2),
            "xz" | "lzma" => Some(Self::Xz),
            "zst" | "zstd" => Some(Self::Zst),
            _ => None,
        }
    }

    /// Returns the canonical file extension for this format.
    pub fn ext(&self) -> &'static str {
        match self {
            Self::Gz => "gz",
            Self::Bz2 => "bz2",
            Self::Xz => "xz",
            Self::Zst => "zst",
        }
    }

    /// Human-readable display name.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Gz => "gzip",
            Self::Bz2 => "bzip2",
            Self::Xz => "xz",
            Self::Zst => "zstd",
        }
    }
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_from_simple_extension() {
        let p = PathBuf::from("archive.gz");
        assert_eq!(Format::from_path(&p), Some(Format::Gz));
    }

    #[test]
    fn format_from_compound_extension() {
        // PathBuf::extension() returns the last extension component,
        // so .tar.gz → "gz" — which is exactly what we want.
        let p = PathBuf::from("archive.tar.xz");
        assert_eq!(Format::from_path(&p), Some(Format::Xz));
    }

    #[test]
    fn format_from_all_aliases() {
        let cases = [
            ("file.gzip", Format::Gz),
            ("file.bzip2", Format::Bz2),
            ("file.lzma", Format::Xz),
            ("file.zstd", Format::Zst),
        ];
        for (name, expected) in cases {
            assert_eq!(
                Format::from_path(Path::new(name)),
                Some(expected),
                "failed for {name}"
            );
        }
    }

    #[test]
    fn format_unknown_extension_returns_none() {
        for name in ["readme.txt", "archive.tar", "file.zip", "Makefile"] {
            assert_eq!(
                Format::from_path(Path::new(name)),
                None,
                "expected None for {name}"
            );
        }
    }

    #[test]
    fn format_no_extension_returns_none() {
        assert_eq!(Format::from_path(Path::new("Makefile")), None);
    }

    #[test]
    fn format_ext_roundtrip() {
        for fmt in [Format::Gz, Format::Bz2, Format::Xz, Format::Zst] {
            assert_eq!(
                Format::from_ext(fmt.ext()),
                Some(fmt),
                "ext() → from_ext() roundtrip failed for {fmt:?}",
            );
        }
    }

    #[test]
    fn format_display_names() {
        assert_eq!(Format::Gz.to_string(), "gzip");
        assert_eq!(Format::Bz2.to_string(), "bzip2");
        assert_eq!(Format::Xz.to_string(), "xz");
        assert_eq!(Format::Zst.to_string(), "zstd");
    }

    #[test]
    fn cli_parses_single_file_invocation() {
        let cli = Cli::try_parse_from(["arc", "file.tar.gz", "-o", "file.tar.zst"]).unwrap();
        assert_eq!(cli.inputs, vec![PathBuf::from("file.tar.gz")]);
        assert_eq!(cli.output, Some(PathBuf::from("file.tar.zst")));
        assert_eq!(cli.level, 6);
        assert_eq!(cli.threads, 1);
        assert!(!cli.keep);
        assert!(!cli.force);
        assert!(!cli.stdout);
        assert!(!cli.dry_run);
        assert!(cli.to.is_none());
        assert!(cli.outdir.is_none());
        assert!(cli.generate.is_none());
    }

    #[test]
    fn cli_defaults_are_sensible() {
        let cli = Cli::try_parse_from(["arc", "a.tar.gz", "a.tar.xz"]).unwrap();
        assert_eq!(cli.level, 6);
        assert_eq!(cli.threads, 1);
        assert!(!cli.keep);
        assert!(!cli.force);
        assert!(!cli.stdout);
        assert!(!cli.dry_run);
    }
    #[test]
    fn cli_parses_stdout_mode() {
        let cli =
            Cli::try_parse_from(["arc", "file.tar.gz", "--stdout", "--format", "zst"]).unwrap();
        assert!(cli.stdout);
        assert_eq!(cli.format, Some(Format::Zst));
        assert!(cli.output.is_none());
        assert!(cli.to.is_none());
    }

    #[test]
    fn cli_rejects_stdout_without_format() {
        assert!(Cli::try_parse_from(["arc", "file.tar.gz", "--stdout"]).is_err());
    }

    #[test]
    fn cli_stdout_accepts_all_formats() {
        for fmt in ["gz", "bz2", "xz", "zst"] {
            assert!(
                Cli::try_parse_from(["arc", "file.tar.gz", "--stdout", "--format", fmt]).is_ok(),
                "--format {fmt} should be accepted in stdout mode"
            );
        }
    }
    #[test]
    fn cli_parses_batch_invocation() {
        let cli = Cli::try_parse_from(["arc", "a.tar.gz", "b.tar.gz", "c.tar.gz", "--to", "zst"])
            .unwrap();
        assert_eq!(cli.inputs.len(), 3);
        assert_eq!(
            cli.inputs,
            vec![
                PathBuf::from("a.tar.gz"),
                PathBuf::from("b.tar.gz"),
                PathBuf::from("c.tar.gz"),
            ]
        );
        assert_eq!(cli.to, Some(Format::Zst));
        assert!(cli.output.is_none());
    }

    #[test]
    fn cli_parses_to_with_outdir() {
        let cli =
            Cli::try_parse_from(["arc", "a.tar.gz", "--to", "xz", "--outdir", "/tmp/out"]).unwrap();
        assert_eq!(cli.to, Some(Format::Xz));
        assert_eq!(cli.outdir, Some(PathBuf::from("/tmp/out")));
    }

    #[test]
    fn cli_rejects_outdir_without_to() {
        assert!(Cli::try_parse_from(["arc", "a.tar.gz", "--outdir", "/tmp/out"]).is_err());
    }

    #[test]
    fn cli_rejects_to_and_format_together() {
        // --to and --format conflict: --format is reserved for --stdout
        assert!(Cli::try_parse_from(["arc", "a.tar.gz", "--to", "zst", "--format", "xz"]).is_err());
    }
    #[test]
    fn cli_parses_generate_bash() {
        let cli = Cli::try_parse_from(["arc", "--generate", "bash"]).unwrap();
        assert!(matches!(cli.generate, Some(Shell::Bash)));
    }

    #[test]
    fn cli_parses_generate_all_shells() {
        for shell in ["bash", "zsh", "fish", "elvish", "powershell"] {
            assert!(
                Cli::try_parse_from(["arc", "--generate", shell]).is_ok(),
                "--generate {shell} should be accepted"
            );
        }
    }

    #[test]
    fn cli_parses_dry_run_single() {
        let cli = Cli::try_parse_from(["arc", "file.tar.gz", "file.tar.zst", "--dry-run"]).unwrap();
        assert!(cli.dry_run);
    }

    #[test]
    fn cli_parses_dry_run_batch() {
        let cli = Cli::try_parse_from(["arc", "a.tar.gz", "b.tar.gz", "--to", "xz", "--dry-run"])
            .unwrap();
        assert!(cli.dry_run);
        assert_eq!(cli.inputs.len(), 2);
    }

    #[test]
    fn cli_rejects_invalid_level() {
        assert!(Cli::try_parse_from(["arc", "a.gz", "a.xz", "--level", "0"]).is_err());
        assert!(Cli::try_parse_from(["arc", "a.gz", "a.xz", "--level", "10"]).is_err());
    }

    #[test]
    fn cli_accepts_all_valid_levels() {
        for l in 1u8..=9 {
            let cli =
                Cli::try_parse_from(["arc", "a.gz", "a.xz", "--level", &l.to_string()]).unwrap();
            assert_eq!(cli.level, l);
        }
    }

    #[test]
    fn cli_threads_zero_accepted() {
        let cli = Cli::try_parse_from(["arc", "a.gz", "a.xz", "--threads", "0"]).unwrap();
        assert_eq!(cli.threads, 0);
    }
    #[test]
    fn cli_parses_all_flags_single() {
        let cli = Cli::try_parse_from([
            "arc",
            "a.tar.gz",
            "-o",
            "a.tar.xz",
            "--level",
            "9",
            "--threads",
            "0",
            "--keep",
            "--force",
            "--dry-run",
        ])
        .unwrap();
        assert_eq!(cli.inputs, vec![PathBuf::from("a.tar.gz")]);
        assert_eq!(cli.output, Some(PathBuf::from("a.tar.xz")));
        assert_eq!(cli.level, 9);
        assert_eq!(cli.threads, 0);
        assert!(cli.keep);
        assert!(cli.force);
        assert!(cli.dry_run);
    }

    #[test]
    fn cli_parses_all_flags_batch() {
        let cli = Cli::try_parse_from([
            "arc",
            "a.tar.gz",
            "b.tar.gz",
            "--to",
            "zst",
            "--outdir",
            "/tmp",
            "--level",
            "3",
            "--threads",
            "4",
            "--keep",
            "--force",
            "--dry-run",
        ])
        .unwrap();
        assert_eq!(cli.inputs.len(), 2);
        assert_eq!(cli.to, Some(Format::Zst));
        assert_eq!(cli.outdir, Some(PathBuf::from("/tmp")));
        assert_eq!(cli.level, 3);
        assert_eq!(cli.threads, 4);
        assert!(cli.keep);
        assert!(cli.force);
        assert!(cli.dry_run);
    }
}
