//! `biolic grep`: pattern search in reads.
//!
//! STATUS: STUB. See Section 5.6 of biolic_plan.md.

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::Args;

use crate::cli::RunContext;

#[derive(Args, Debug)]
pub struct GrepArgs {
    pub input: PathBuf,
    #[arg(short = 'p', long)]
    pub pattern: Option<String>,
    #[arg(short = 'f', long)]
    pub patterns_file: Option<PathBuf>,
    #[arg(long)]
    pub regex: bool,
    #[arg(short = 'v', long)]
    pub invert: bool,
    #[arg(short = 'n', long)]
    pub search_names: bool,
    #[arg(long)]
    pub both_strands: bool,
}

pub fn run(_args: GrepArgs, _ctx: &RunContext) -> Result<()> {
    Err(anyhow!(
        "biolic grep is not yet implemented. See Section 5.6."
    ))
}
