//! Backend detection, installation guidance, and subprocess plumbing.
//!
//! Each compression format is served by one or two external binaries:
//!   - a primary binary (parallel-capable)
//!   - an optional fallback (single-threaded, usually pre-installed)
//!
//! `resolve_backend` picks the best available binary for a given
//! (format, role) pair at runtime. If nothing is found it returns a
//! structured `MissingBackend` error with install instructions.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use anyhow::{bail, Context, Result};
use which::which;

use crate::cli::Format;

// Backend descriptors ==========================================================

/// Whether we are compressing or decompressing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Compress,
    Decompress,
}

/// A resolved executable + the base argv that should precede user flags.
#[derive(Debug, Clone)]
pub struct Backend {
    /// Absolute path to the binary (resolved via PATH).
    pub path: PathBuf,
    /// Human-readable name used in log messages.
    pub name: &'static str,
    /// Whether this backend supports parallel execution.
    pub parallel: bool,
}

// Install instructions section =================================================

/// Operating system family, detected at compile time.
#[derive(Debug, Clone, Copy)]
enum Os {
    Linux,
    Macos,
    Other,
}

/// Detects the current operating system family.
fn current_os() -> Os {
    if cfg!(target_os = "macos") {
        Os::Macos
    } else if cfg!(target_os = "linux") {
        Os::Linux
    } else {
        Os::Other
    }
}

/// Returns installation hints (primary_pkg, fallback_pkg, brew_pkg) for each tool.
fn install_hint(binary: &str, os: Os) -> String {
    struct Hint {
        apt: Option<&'static str>,
        dnf: Option<&'static str>,
        brew: Option<&'static str>,
        note: Option<&'static str>,
    }

    let h: Hint = match binary {
        "pigz" => Hint {
            apt: Some("sudo apt install pigz"),
            dnf: Some("sudo dnf install pigz"),
            brew: Some("brew install pigz"),
            note: None,
        },
        "pbzip2" => Hint {
            apt: Some("sudo apt install pbzip2"),
            dnf: Some("sudo dnf install pbzip2"),
            brew: Some("brew install pbzip2"),
            note: None,
        },
        "gzip" => Hint {
            apt: Some("sudo apt install gzip"),
            dnf: Some("sudo dnf install gzip"),
            brew: Some("brew install gzip"),
            note: None,
        },
        "bzip2" => Hint {
            apt: Some("sudo apt install bzip2"),
            dnf: Some("sudo dnf install bzip2"),
            brew: Some("brew install bzip2"),
            note: None,
        },
        "xz" => Hint {
            apt: Some("sudo apt install xz-utils"),
            dnf: Some("sudo dnf install xz"),
            brew: Some("brew install xz"),
            note: Some("xz >= 5.2 supports multithreading natively via -T."),
        },
        "zstd" => Hint {
            apt: Some("sudo apt install zstd"),
            dnf: Some("sudo dnf install zstd"),
            brew: Some("brew install zstd"),
            note: Some("zstd supports multithreading natively via -T."),
        },
        _ => Hint {
            apt: None,
            dnf: None,
            brew: None,
            note: None,
        },
    };

    let cmd = match os {
        Os::Macos => h
            .brew
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("# No known install command for '{binary}' on macOS")),
        Os::Linux => {
            // Detect package manager presence at runtime.
            let has_apt = which("apt").is_ok();
            let has_dnf = which("dnf").is_ok();
            match (has_apt, has_dnf) {
                (true, _) => h
                    .apt
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("# No apt package known for '{binary}'")),
                (_, true) => h
                    .dnf
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("# No dnf package known for '{binary}'")),
                _ => format!("# Could not detect package manager. Install '{binary}' manually."),
            }
        }
        Os::Other => format!("# Install '{binary}' using your system package manager."),
    };

    let mut out = cmd;
    if let Some(note) = h.note {
        out.push('\n');
        out.push_str(note);
    }
    out
}

// Backend resolution ==========================================================

/// Candidate: (binary_name, is_parallel, is_required).
///
/// Candidates are tried left-to-right; the first one found on PATH wins.
/// If `required` is true and the binary is not found, an error is returned.
type Candidates = &'static [(&'static str, bool)];

/// Returns the candidates for a given format and role.
fn candidates_for(format: Format, _role: Role) -> Candidates {
    // Decompression and compression use the same binaries for all formats,
    // so role is unused for now but kept for future specialisation
    // (e.g. lbzip2 decompresses bz2 faster than pbzip2).
    match format {
        Format::Gz => &[
            ("pigz", true),  // parallel, preferred
            ("gzip", false), // fallback, almost always present
        ],
        Format::Bz2 => &[
            ("pbzip2", true), // parallel, preferred
            ("bzip2", false), // fallback
        ],
        Format::Xz => &[
            // xz >= 5.2 has -T built-in; no separate parallel binary needed.
            ("xz", true),
        ],
        Format::Zst => &[
            // zstd has -T built-in.
            ("zstd", true),
        ],
    }
}

/// Resolve the best available backend for `format` + `role`.
///
/// Returns `Ok(Backend)` with the first candidate found on PATH, or an
/// `Err` describing what is missing and how to install it.
pub fn resolve_backend(format: Format, role: Role) -> Result<Backend> {
    let candidates = candidates_for(format, role);

    for &(name, parallel) in candidates {
        if let Ok(path) = which(name) {
            log::debug!("Resolved backend for {format} ({role:?}): {name} at {path:?}");
            return Ok(Backend {
                path,
                name,
                parallel,
            });
        }
    }

    // Nothing found, build a helpful error.
    let os = current_os();
    let missing: Vec<&str> = candidates.iter().map(|(n, _)| *n).collect();
    let primary = missing[0];
    let hint = install_hint(primary, os);

    bail!(
        "No backend found for {format} compression.\n\
         Tried: {tried}\n\n\
         To install '{primary}':\n\
         {hint}",
        tried = missing.join(", "),
    )
}

// Thread count helper =========================================================

/// Resolve the actual thread count to pass to backends.
///
/// `0` means "use all logical cores" (same convention as xz/zstd -T0).
pub fn resolve_threads(requested: u32) -> u32 {
    if requested == 0 {
        std::thread::available_parallelism()
            .map(|n| n.get() as u32)
            .unwrap_or(1)
    } else {
        requested
    }
}

// Subprocess builders =========================================================

/// Build a decompressor child process that reads from `stdin_src` and
/// writes decompressed bytes to its stdout (which the caller pipes into
/// the compressor).
pub fn spawn_decompressor(backend: &Backend, format: Format, stdin_src: Stdio) -> Result<Child> {
    let mut cmd = Command::new(&backend.path);

    match format {
        Format::Gz => {
            // pigz -dc  /  gzip -dc
            cmd.args(["-d", "-c"]);
        }
        Format::Bz2 => {
            // pbzip2 -dc  /  bzip2 -dc
            cmd.args(["-d", "-c"]);
        }
        Format::Xz => {
            // xz -dc
            cmd.args(["-d", "-c"]);
        }
        Format::Zst => {
            // zstd -dc
            cmd.args(["-d", "-c"]);
        }
    }

    cmd.stdin(stdin_src)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("Failed to spawn decompressor '{}'", backend.name))
}

/// Build a compressor child process that reads uncompressed bytes from
/// `stdin_src` and writes to its stdout (which the caller redirects to the
/// output file).
pub fn spawn_compressor(
    backend: &Backend,
    format: Format,
    level: u8,
    threads: u32,
    stdin_src: Stdio,
) -> Result<Child> {
    let mut cmd = Command::new(&backend.path);
    let level_flag = format!("-{level}");

    match format {
        Format::Gz => {
            // pigz -c -N  /  gzip -c
            // pigz accepts -p <threads>; gzip ignores extra flags safely.
            cmd.args(["-c", &level_flag]);
            if backend.parallel && threads > 1 {
                cmd.args(["-p", &threads.to_string()]);
            }
        }
        Format::Bz2 => {
            // pbzip2 -c -p<threads>  /  bzip2 -c
            cmd.args(["-c", &level_flag]);
            if backend.parallel && threads > 1 {
                cmd.args([&format!("-p{threads}")]);
            }
        }
        Format::Xz => {
            // xz -c -T<threads>
            cmd.args(["-c", &level_flag, &format!("-T{threads}")]);
        }
        Format::Zst => {
            // zstd -c -T<threads>
            // zstd level scale is 1-19; we clamp 1-9 to a sensible sub-range.
            let zstd_level = map_level_to_zstd(level);
            cmd.args(["-c", &format!("-{zstd_level}"), &format!("-T{threads}")]);
        }
    }

    cmd.stdin(stdin_src)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("Failed to spawn compressor '{}'", backend.name))
}

/// Map arc's 1-9 level scale to zstd's 1-19 scale.
///
/// zstd's useful range is roughly 1-19 (with 3 being the default).
/// We spread our 9 steps across 1-19 to preserve relative intent:
///   arc 1 → zstd 1, arc 5 → zstd 9, arc 9 → zstd 19
fn map_level_to_zstd(level: u8) -> u8 {
    // linear interpolation: out = 1 + (level - 1) * 18 / 8
    let out = 1u8 + ((level as u16 - 1) * 18 / 8) as u8;
    out.clamp(1, 19)
}

// Logging setup ===============================================================

pub fn setup_logging(verbosity: u8) -> Result<()> {
    let level = match verbosity {
        0 => log::LevelFilter::Warn,
        1 => log::LevelFilter::Info,
        2 => log::LevelFilter::Debug,
        _ => log::LevelFilter::Trace,
    };

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{level}] {target}: {message}",
                level = record.level(),
                target = record.target(),
            ))
        })
        .level(level)
        .chain(std::io::stderr())
        .apply()
        .context("Failed to initialise logging")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zstd_level_mapping_bounds() {
        assert_eq!(map_level_to_zstd(1), 1);
        assert_eq!(map_level_to_zstd(9), 19);
    }

    #[test]
    fn zstd_level_mapping_midpoint() {
        // level 5 should land around the middle of zstd's range
        let mid = map_level_to_zstd(5);
        assert!(mid >= 8 && mid <= 11, "mid={mid}");
    }

    #[test]
    fn resolve_threads_zero_returns_positive() {
        assert!(resolve_threads(0) >= 1);
    }

    #[test]
    fn resolve_threads_explicit_passthrough() {
        assert_eq!(resolve_threads(4), 4);
    }

    #[test]
    fn resolve_backend_xz_found() {
        // xz is almost universally installed; skip if not available in CI.
        if which("xz").is_err() {
            return;
        }
        let b = resolve_backend(Format::Xz, Role::Compress).unwrap();
        assert_eq!(b.name, "xz");
        assert!(b.parallel);
    }

    #[test]
    fn resolve_backend_missing_gives_hint() {
        // Test the error path with a format whose primary binary is unlikely
        // to be installed in a minimal CI environment (pbzip2).
        // We just verify the error message is non-empty and contains the name.
        if which("pbzip2").is_err() && which("bzip2").is_err() {
            let err = resolve_backend(Format::Bz2, Role::Compress).unwrap_err();
            let msg = err.to_string();
            assert!(msg.contains("bzip2") || msg.contains("pbzip2"));
            assert!(msg.contains("install") || msg.contains("Install"));
        }
    }
}
