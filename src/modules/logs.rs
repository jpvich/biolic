//! `biolic logs`: execution history and telemetry (ORIGINAL CONTRIBUTION).
//!
//! STATUS: STUB. This is the second novel module, described in Section 5.9
//! of biolic_plan.md. Persists every biolic execution to a SQLite database
//! at ~/.biolic/logs.db for later querying and aggregation.

use anyhow::{anyhow, Result};
use clap::{Args, Subcommand};

use crate::cli::RunContext;

#[derive(Args, Debug)]
pub struct LogsArgs {
    #[command(subcommand)]
    pub command: Option<LogsCommand>,

    /// Show the last N entries (default 20).
    #[arg(long, default_value = "20")]
    pub last: usize,
}

#[derive(Subcommand, Debug)]
pub enum LogsCommand {
    /// Summarize logs (overall or grouped).
    Summary {
        #[arg(long)]
        by_module: bool,
        #[arg(long)]
        by_month: bool,
    },
    /// Export logs to a file format.
    Export {
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Delete entries older than a given duration.
    Prune {
        #[arg(long)]
        older_than: String,
    },
    /// Delete all entries (with confirmation).
    Clear,
    /// Print the path to the logs database.
    Path,
}

pub fn run(_args: LogsArgs, _ctx: &RunContext) -> Result<()> {
    Err(anyhow!(
        "biolic logs is not yet implemented. See Section 5.9 of biolic_plan.md. \
         Backend: SQLite at ~/.biolic/logs.db. Implement schema migrations first."
    ))
}
