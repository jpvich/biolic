//! `biolic sample`: subsample reads.
//!
//! STATUS: STUB. See Section 5.5 of biolic_plan.md.

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::Args;

use crate::cli::RunContext;

#[derive(Args, Debug)]
pub struct SampleArgs {
    pub input: PathBuf,
    #[arg(short = 'n', long, conflicts_with_all = &["proportion", "bases", "coverage"])]
    pub count: Option<u64>,
    #[arg(short = 'p', long, conflicts_with_all = &["count", "bases", "coverage"])]
    pub proportion: Option<f64>,
    #[arg(long, conflicts_with_all = &["count", "proportion", "coverage"])]
    pub bases: Option<String>,
    #[arg(long, requires = "genome_size")]
    pub coverage: Option<f64>,
    #[arg(long)]
    pub genome_size: Option<String>,
    #[arg(long)]
    pub seed: Option<u64>,
}

pub fn run(_args: SampleArgs, _ctx: &RunContext) -> Result<()> {
    Err(anyhow!(
        "biolic sample is not yet implemented. See Section 5.5."
    ))
}
