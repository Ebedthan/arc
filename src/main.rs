mod cli;
mod error;
mod utils;

use clap::Parser;
use cli::Cli;
use std::env;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::process;

use anyhow::{Context, Result};
use log::info;
use niffler::get_writer;

fn main() -> Result<()> {
    // Reading command line arguments
    let cli = Cli::parse();

    process::exit(exitcode::OK)
}
