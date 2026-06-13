//! biolic: Bioinformatics Integrated Operations Library for IO & Computation.
//!
//! Binary entry point. All logic lives in the library crate (`src/lib.rs`).

use anyhow::Result;
use clap::Parser;

use biolic::cli::Cli;

fn main() -> Result<()> {
    let cli = Cli::parse();
    biolic::cli::run(cli)
}
