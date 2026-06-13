//! `biolic tail`: extract last N reads or bases.
//!
//! STATUS: STUB. See Section 5.7 of biolic_plan.md.

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::Args;

use crate::cli::RunContext;

#[derive(Args, Debug)]
pub struct TailArgs {
    pub input: PathBuf,
    #[arg(short = 'n', long, default_value = "10")]
    pub reads: Option<u64>,
    #[arg(long)]
    pub bases: Option<String>,
}

pub fn run(_args: TailArgs, _ctx: &RunContext) -> Result<()> {
    Err(anyhow!(
        "biolic tail is not yet implemented. See Section 5.7."
    ))
}
