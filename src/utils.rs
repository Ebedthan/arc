extern crate anyhow;
extern crate chrono;
extern crate fern;
extern crate log;
extern crate niffler;

use std::fs::File;
use std::io;

use anyhow::{anyhow, Context, Result};

use crate::error;

pub fn setup_logging(verbosity: u64) -> Result<(), fern::InitError> {
    let mut base_config = fern::Dispatch::new();

    base_config = match verbosity {
        0 => {
            // No message will be outputed as in quiet mode
            base_config
                .level(log::LevelFilter::Info)
                .level_for("overly-verbose-target", log::LevelFilter::Warn)
        }
        1 => base_config
            .level(log::LevelFilter::Debug)
            .level_for("overly-verbose-target", log::LevelFilter::Info),
        2 => base_config.level(log::LevelFilter::Debug),
        _3_or_more => base_config.level(log::LevelFilter::Trace),
    };

    let file_config = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}[{}][{}] {}",
                chrono::Local::now().format("[%H:%M:%S]"),
                record.target(),
                record.level(),
                message
            ))
        })
        .chain(fern::log_file("arc.log")?);

    let stdout_config = fern::Dispatch::new()
        .format(|out, message, record| {
            // special format for debug messages from arc
            if record.level() > log::LevelFilter::Info
                && record.target() == "arc"
            {
                out.finish(format_args!(
                    "---\nDEBUG: {}: {}\n---",
                    chrono::Local::now().format("%H:%M:%S"),
                    message
                ))
            } else {
                out.finish(format_args!(
                    "[{}][{}][{}] {}",
                    chrono::Local::now().format("%H:%M:%S"),
                    record.target(),
                    record.level(),
                    message
                ))
            }
        })
        .chain(io::stdout());

    base_config
        .chain(file_config)
        .chain(stdout_config)
        .apply()?;

    Ok(())
}

// to_niffler_level function

/// Convert an integer to a niffler::Level
///
/// # Example
/// ```rust
/// assert_eq!(to_niffler_level(3), niffler::Level::Three);
/// ```
///
pub fn to_niffler_level(level: &str) -> niffler::Level {
    match level {
        "1" => niffler::Level::One,
        "2" => niffler::Level::Two,
        "3" => niffler::Level::Three,
        "4" => niffler::Level::Four,
        "5" => niffler::Level::Five,
        "6" => niffler::Level::Six,
        "7" => niffler::Level::Seven,
        "8" => niffler::Level::Eight,
        "9" => niffler::Level::Nine,
        _ => niffler::Level::One,
    }
}

// to_niffler_format function
pub fn to_niffler_format(format: &str) -> Result<niffler::compression::Format> {
    match format {
        "gz" => Ok(niffler::compression::Format::Gzip),
        "bz2" => Ok(niffler::compression::Format::Bzip),
        "xz" => Ok(niffler::compression::Format::Lzma),
        _ => Ok(niffler::compression::Format::No),
    }
}

#[derive(Debug, PartialEq)]
pub enum FileType {
    Gzip,
    Lzma,
    Bzip2,
    None,
}

pub fn read_file(
    filename: &str,
) -> Result<(Box<dyn io::Read>, niffler::compression::Format)> {
    let raw_in = Box::new(io::BufReader::new(
        File::open(filename).with_context(|| error::Error::CantReadFile {
            filename: filename.to_string(),
        })?,
    ));

    niffler::get_reader(raw_in).with_context(|| {
        anyhow!("Could not detect compression of file '{}'", filename)
    })
}

pub fn last_newline(s: &[u8]) -> usize {
    let mut i = s.len() - 1;
    while i > 0 {
        if s[i] == b'\n' {
            return i + 1;
        }
        i -= 1;
    }
    s.len()
}

pub fn process_line(line: &[u8]) {
    println!("{:?}", line);
}
