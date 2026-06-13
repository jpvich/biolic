//! `biolic convert`: format conversion (FASTQ/FASTA/BAM).
//!
//! STATUS: STUB. See Section 5.4 of biolic_plan.md.

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::Args;

use crate::cli::RunContext;

#[derive(Args, Debug)]
pub struct ConvertArgs {
    pub input: PathBuf,
    #[arg(short = 'o', long)]
    pub output: PathBuf,
    #[arg(long)]
    pub keep_tags: bool,
}

pub fn run(_args: ConvertArgs, _ctx: &RunContext) -> Result<()> {
    Err(anyhow!(
        "biolic convert is not yet implemented. See Section 5.4."
    ))
}
