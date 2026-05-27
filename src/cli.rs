use std::path::Path;
use std::path::PathBuf;

use clap::{Parser, ValueEnum};

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
    /// Input file. Format is detected from the file extension.
    ///
    /// Supported input formats: .gz, .bz2, .xz, .zst
    /// Compound extensions are also supported: .tar.gz, .tar.bz2, .tar.xz, .tar.zst
    pub input: PathBuf,

    /// Output file. Target format is inferred from the extension.
    ///
    /// Supported output formats: .gz, .bz2, .xz, .zst
    pub output: PathBuf,

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

    /// Increase log verbosity. Use multiple times for more detail (-v, -vv, -vvv).
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
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
        // so .tar.gz → "gz".
        let p = PathBuf::from("archive.tar.xz");
        assert_eq!(Format::from_path(&p), Some(Format::Xz));
    }

    #[test]
    fn format_from_zst_alias() {
        let p = PathBuf::from("data.zstd");
        assert_eq!(Format::from_path(&p), Some(Format::Zst));
    }

    #[test]
    fn format_unknown_extension_returns_none() {
        let p = PathBuf::from("readme.txt");
        assert_eq!(Format::from_path(&p), None);
    }

    #[test]
    fn format_ext_roundtrip() {
        for fmt in [Format::Gz, Format::Bz2, Format::Xz, Format::Zst] {
            assert_eq!(
                Format::from_ext(fmt.ext()),
                Some(fmt),
                "ext() → from_ext() roundtrip failed for {:?}",
                fmt
            );
        }
    }

    #[test]
    fn cli_parses_basic_invocation() {
        let cli = Cli::try_parse_from(["arc", "file.tar.gz", "file.tar.zst"]).unwrap();
        assert_eq!(cli.input, PathBuf::from("file.tar.gz"));
        assert_eq!(cli.output, PathBuf::from("file.tar.zst"));
        assert_eq!(cli.level, 6);
        assert_eq!(cli.threads, 1);
        assert!(!cli.keep);
        assert!(!cli.force);
    }

    #[test]
    fn cli_parses_all_flags() {
        let cli = Cli::try_parse_from([
            "arc",
            "a.gz",
            "a.xz",
            "--level",
            "9",
            "--threads",
            "0",
            "--keep",
            "--force",
            "-vvv",
        ])
        .unwrap();
        assert_eq!(cli.level, 9);
        assert_eq!(cli.threads, 0);
        assert!(cli.keep);
        assert!(cli.force);
        assert_eq!(cli.verbose, 3);
    }

    #[test]
    fn cli_rejects_invalid_level() {
        assert!(Cli::try_parse_from(["arc", "a.gz", "a.xz", "--level", "10"]).is_err());
        assert!(Cli::try_parse_from(["arc", "a.gz", "a.xz", "--level", "0"]).is_err());
    }
}
