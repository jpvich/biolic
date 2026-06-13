//! `biolic count`: fast read and base counting.
//!
//! Fully implemented. This is the fastest module in biolic — no quality
//! computation, no GC, no allocations beyond the parser internals.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;

use crate::cli::RunContext;
use crate::io::reader::open_input;
use crate::output::{write, HumanDisplay, OutputFormat};

#[derive(Args, Debug)]
pub struct CountArgs {
    /// Input files (aggregated as one stream). With no file, reads stdin.
    pub inputs: Vec<PathBuf>,

    /// Output in JSON format.
    #[arg(long, conflicts_with = "tsv")]
    pub json: bool,

    /// Output in TSV format.
    #[arg(long, conflicts_with = "json")]
    pub tsv: bool,
}

#[derive(Debug, Serialize)]
pub struct CountResult {
    pub file: String,
    pub reads: u64,
    pub bases: u64,
}

pub fn run(args: CountArgs, _ctx: &RunContext) -> Result<()> {
    let result = compute(&args)?;

    let format = if args.json {
        OutputFormat::Json
    } else if args.tsv {
        OutputFormat::Tsv
    } else {
        OutputFormat::auto()
    };

    write(&result, format)?;
    Ok(())
}

fn compute(args: &CountArgs) -> Result<CountResult> {
    let inputs = if args.inputs.is_empty() {
        vec![PathBuf::from("-")]
    } else {
        args.inputs.clone()
    };

    let mut reads = 0u64;
    let mut bases = 0u64;

    for input in &inputs {
        let mut reader =
            open_input(input).with_context(|| format!("opening input {}", input.display()))?;
        while let Some(record) = reader.next_record().context("reading record")? {
            reads += 1;
            bases += record.len() as u64;
        }
    }

    Ok(CountResult {
        file: inputs
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(","),
        reads,
        bases,
    })
}

impl HumanDisplay for CountResult {
    fn write_human(&self, w: &mut dyn std::io::Write) -> Result<()> {
        writeln!(
            w,
            "{}\treads={}\tbases={}",
            self.file, self.reads, self.bases
        )?;
        Ok(())
    }

    fn write_tsv(&self, w: &mut dyn std::io::Write) -> Result<()> {
        writeln!(w, "file\treads\tbases")?;
        writeln!(w, "{}\t{}\t{}", self.file, self.reads, self.bases)?;
        Ok(())
    }
}
