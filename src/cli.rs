//! Top-level CLI definition.
//!
//! Each subcommand dispatches to a module in the `modules` directory.
//! When adding a new module:
//! 1. Add it to `src/modules/mod.rs`
//! 2. Add a variant to the `Commands` enum below
//! 3. Add the dispatch in `run()`

use std::io::IsTerminal;

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::modules;

/// biolic: a modular bioinformatics toolkit in Rust.
#[derive(Parser, Debug)]
#[command(
    name = "biolic",
    version,
    author,
    about = "biolic: Bioinformatics Integrated Operations Library for IO & Computation",
    long_about = "biolic (Bioinformatics Integrated Operations Library for IO & Computation) \
                  is a fast, memory-efficient, general-purpose toolkit for sequencing data \
                  (FASTQ, FASTA, and unaligned BAM), built as a platform that grows through \
                  modules. Streaming-first design guarantees constant memory regardless of \
                  input size, with particularly strong support for long reads (Oxford \
                  Nanopore and PacBio HiFi)."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Disable execution logging for this run.
    #[arg(long, global = true)]
    pub no_log: bool,

    /// Suppress progress output and informational messages.
    /// (Long-only: `-q` is reserved for per-command quality, e.g. `filter -q`.)
    #[arg(long, global = true)]
    pub quiet: bool,

    /// Enable verbose output (info-level logging to stderr).
    /// (Long-only: `-v` is reserved for per-command use, e.g. `grep -v` invert.)
    #[arg(long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Compute summary statistics for sequence files (N50, length distribution, quality).
    Stats(modules::stats::StatsArgs),

    /// Count reads and bases quickly (fastest module in biolic).
    Count(modules::count::CountArgs),

    /// Filter reads by length, quality, GC content, or N percentage.
    Filter(modules::filter::FilterArgs),

    /// Convert between sequence formats (FASTQ, FASTA, BAM).
    Convert(modules::convert::ConvertArgs),

    /// Subsample reads by count, proportion, or target coverage.
    Sample(modules::sample::SampleArgs),

    /// Search for sequence patterns in reads.
    Grep(modules::grep::GrepArgs),

    /// Extract the first N reads or bases.
    Head(modules::head::HeadArgs),

    /// Extract the last N reads or bases.
    Tail(modules::tail::TailArgs),

    /// Adaptive QC with mixture models, anomaly detection, and threshold recommendations.
    Qc(modules::qc::QcArgs),

    /// Query and manage execution logs.
    Logs(modules::logs::LogsArgs),

    /// Generate a shell completion script (bash, zsh, fish, ...).
    Completions(CompletionsArgs),
}

#[derive(clap::Args, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate a completion script for.
    pub shell: clap_complete::Shell,
}

/// Dispatch the parsed CLI to the appropriate module.
pub fn run(cli: Cli) -> Result<()> {
    let context = RunContext {
        no_log: cli.no_log,
        quiet: cli.quiet,
        verbose: cli.verbose,
    };

    // No subcommand: show the banner. On an interactive terminal, drop into the
    // REPL; when piped / non-interactive, just print the banner and exit.
    let Some(command) = cli.command else {
        crate::banner::print_banner();
        if std::io::stdin().is_terminal() {
            crate::repl::run_repl(&context)?;
        }
        return Ok(());
    };

    dispatch(command, &context)
}

/// Dispatch a parsed subcommand to its module. Shared by one-shot `run` and the REPL.
pub fn dispatch(command: Commands, context: &RunContext) -> Result<()> {
    match command {
        Commands::Stats(args) => modules::stats::run(args, context),
        Commands::Count(args) => modules::count::run(args, context),
        Commands::Filter(args) => modules::filter::run(args, context),
        Commands::Convert(args) => modules::convert::run(args, context),
        Commands::Sample(args) => modules::sample::run(args, context),
        Commands::Grep(args) => modules::grep::run(args, context),
        Commands::Head(args) => modules::head::run(args, context),
        Commands::Tail(args) => modules::tail::run(args, context),
        Commands::Qc(args) => modules::qc::run(args, context),
        Commands::Logs(args) => modules::logs::run(args, context),
        Commands::Completions(args) => {
            use clap::CommandFactory;
            let mut cmd = Cli::command();
            clap_complete::generate(args.shell, &mut cmd, "biolic", &mut std::io::stdout());
            Ok(())
        }
    }
}

/// Shared runtime context passed to every module.
///
/// Holds the global flags from the top-level CLI.
#[derive(Debug, Clone)]
pub struct RunContext {
    pub no_log: bool,
    pub quiet: bool,
    pub verbose: bool,
}
