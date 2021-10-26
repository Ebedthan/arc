extern crate anyhow;
extern crate log;
extern crate rayon;

mod app;
mod error;
mod utils;

use std::env;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::process;

use anyhow::{Context, Result};
use log::info;
use niffler::get_writer;

const CHUNK_SIZE: usize = 100_000_000;

fn main() -> Result<()> {
    // Define command-line arguments ----------------------------------------
    let matches = app::build_app().get_matches_from(env::args_os());

    // Reading command line arguments
    let infile = matches.value_of("FILE").with_context(|| "Input file could not ne found. Is the path correct, readable or with correct permission?")?;
    let filetype = matches.value_of("type").unwrap_or("gz");
    let level = matches.value_of("level").unwrap();
    let verbosity: u64 = matches.occurrences_of("verbose");

    // Converting level to niffler level
    let niffler_level = utils::to_niffler_level(level);

    // Setting up logging
    utils::setup_logging(verbosity).expect("failed to initialize logging");

    // Get input path stem
    let file_stem = Path::new(infile).file_stem().unwrap();

    // Decompress input file into temporary file
    info!("Decompressing file");
    let (mut reader, _compression) = utils::read_file(infile)?;
    let mut buffer = Vec::with_capacity(CHUNK_SIZE);
    let decompressed = File::create(file_stem)?;

    loop {
        // Read at most CHUNK_SIZE: 100 MB into buffer.
        reader
            .by_ref()
            .take((CHUNK_SIZE - buffer.len()) as u64)
            .read_to_end(&mut buffer)
            .unwrap();

        if buffer.len() == 0 {
            // The file has ended.
            break;
        }

        // Copy any incomplete lines to the next buffer.
        let last_newline = utils::last_newline(&buffer);
        let mut next_buf = Vec::with_capacity(CHUNK_SIZE);
        next_buf.extend_from_slice(&buffer[last_newline..]);
        buffer.truncate(last_newline);

        // Write buffer content to file.
        decompressed.write_all(&buffer)?;
        buffer = next_buf;
    }

    info!("Done decompressing");

    let compressed_filename = Path::new(format!("{:?}.{}", file_stem, filetype));
    let out_compression = utils::to_niffler_format(filetype)?;
    rayon::scope(|scope| {
        let mut fd = File::create(compressed_filename).unwrap();
        let mut out_writer = get_writer(Box::new(fd), out_compression, niffler_level);
        let mut s = Vec::with_capacity(CHUNK_SIZE);

        loop {
            &fd.take((CHUNK_SIZE - s.len()) as u64).read_to_end(&mut s).unwrap();

            if s.len() == 0 {
                break;
            }

            let last_newline = utils::last_newline(&s);
            let mut next_s = Vec::with_capacity(CHUNK_SIZE);
            next_s.extend_from_slice(&s[last_newline..]);
            s.truncate(last_newline);

            let data = s;
            scope.spawn(move |_| {
                data[..last_newline]
                    .split(|c| *c == b'\n')
                    .par_bridge()
                    .for_each(utils::process_line);
            });

            s = next_s;
        }
    });

    process::exit(exitcode::OK)
}
