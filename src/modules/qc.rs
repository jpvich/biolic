//! `biolic qc`: adaptive quality control (ORIGINAL CONTRIBUTION).
//!
//! STATUS: STUB. This is the novel module described in Section 5.8 of
//! biolic_plan.md. Uses mixture models, change-point detection, and
//! techniques from quantitative finance to interpret sequencing data
//! rather than just report raw statistics.

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::Args;

use crate::cli::RunContext;

#[derive(Args, Debug)]
pub struct QcArgs {
    pub input: PathBuf,
    #[arg(long)]
    pub output: Option<PathBuf>,
    #[arg(long)]
    pub suggest_thresholds: bool,
    #[arg(long)]
    pub platform: Option<String>,
    #[arg(long)]
    pub anomalies_only: bool,
    #[arg(long)]
    pub report_energy: bool,
    #[arg(long)]
    pub json: bool,
}

pub fn run(_args: QcArgs, _ctx: &RunContext) -> Result<()> {
    Err(anyhow!(
        "biolic qc is not yet implemented. This is the novel module (Section 5.8). \
         Implementation order: 1) Mixture model fitting, 2) Threshold recommendation, \
         3) HTML report, 4) Change-point detection, 5) Platform baselining, 6) Energy reporting."
    ))
}
