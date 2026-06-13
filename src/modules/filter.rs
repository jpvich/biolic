//! `biolic filter`: filter reads by length, quality, GC, etc.
//!
//! STATUS: STUB. To implement, see Section 5.2 of biolic_plan.md.
//!
//! Reference tools to study:
//! - chopper (Rust): https://github.com/wdecoster/chopper
//! - NanoFilt (Python): https://github.com/wdecoster/nanofilt

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::Args;

use crate::cli::RunContext;

#[derive(Args, Debug)]
pub struct FilterArgs {
    /// Input file. Use "-" for stdin.
    pub input: PathBuf,

    /// Minimum read length to keep.
    #[arg(short = 'l', long)]
    pub min_length: Option<u64>,

    /// Maximum read length to keep.
    #[arg(short = 'm', long)]
    pub max_length: Option<u64>,

    /// Minimum mean quality (Phred) to keep.
    #[arg(short = 'q', long)]
    pub min_quality: Option<f64>,

    /// Maximum N base percentage allowed.
    #[arg(long)]
    pub max_n: Option<f64>,

    /// Minimum GC content (0.0-1.0).
    #[arg(long)]
    pub min_gc: Option<f64>,

    /// Maximum GC content (0.0-1.0).
    #[arg(long)]
    pub max_gc: Option<f64>,

    /// Remove N bases from the start of each read.
    #[arg(long)]
    pub headcrop: Option<usize>,

    /// Remove N bases from the end of each read.
    #[arg(long)]
    pub tailcrop: Option<usize>,
}

pub fn run(_args: FilterArgs, _ctx: &RunContext) -> Result<()> {
    Err(anyhow!(
        "biolic filter is not yet implemented. See Section 5.2 of biolic_plan.md."
    ))
}
