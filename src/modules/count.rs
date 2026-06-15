//! `biolic count`: fast read and base counting.
//!
//! Fully implemented. This is the fastest module in biolic — no quality
//! computation, no GC, no allocations beyond the parser internals.
//!
//! Like `stats`, each input file is counted on its own row by default;
//! `--combine` aggregates all inputs into a single row.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;

use crate::cli::RunContext;
use crate::io::reader::open_input;
use crate::output::{write_rows, Cell, Column, OutputFormat, Tabular};

#[derive(Args, Debug)]
pub struct CountArgs {
    /// Input files. Each file is counted on its own row. With no file, reads stdin.
    pub inputs: Vec<PathBuf>,

    /// Output in JSON format.
    #[arg(long, conflicts_with = "tsv")]
    pub json: bool,

    /// Output in TSV format.
    #[arg(long, conflicts_with = "json")]
    pub tsv: bool,

    /// Aggregate all inputs into a single combined row instead of one per file.
    #[arg(long)]
    pub combine: bool,

    /// Show only the file's basename in the `file` column.
    #[arg(short = 'b', long)]
    pub basename: bool,
}

#[derive(Debug, Serialize)]
pub struct CountResult {
    pub file: String,
    pub reads: u64,
    pub bases: u64,
}

pub fn run(args: CountArgs, _ctx: &RunContext) -> Result<()> {
    let rows = compute(&args.inputs, args.combine, args.basename)?;
    let format = OutputFormat::from_flags(args.json, args.tsv);
    write_rows(&rows, format)?;
    Ok(())
}

fn resolved_inputs(inputs: &[PathBuf]) -> Vec<PathBuf> {
    if inputs.is_empty() {
        vec![PathBuf::from("-")]
    } else {
        inputs.to_vec()
    }
}

fn label_for(p: &Path, basename: bool) -> String {
    if basename {
        p.file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| p.display().to_string())
    } else {
        p.display().to_string()
    }
}

/// Pure core (no `clap`, no I/O beyond reading the inputs): future Python
/// bindings call this directly. `run` is the thin CLI wrapper around it.
pub fn compute(inputs: &[PathBuf], combine: bool, basename: bool) -> Result<Vec<CountResult>> {
    let inputs = resolved_inputs(inputs);

    if combine {
        Ok(vec![count_inputs(&inputs, "total".to_string())?])
    } else {
        inputs
            .iter()
            .map(|input| count_inputs(std::slice::from_ref(input), label_for(input, basename)))
            .collect()
    }
}

fn count_inputs(inputs: &[PathBuf], label: String) -> Result<CountResult> {
    let mut reads = 0u64;
    let mut bases = 0u64;

    for input in inputs {
        let mut reader =
            open_input(input).with_context(|| format!("opening input {}", input.display()))?;
        while let Some(record) = reader.next_record().context("reading record")? {
            reads += 1;
            bases += record.len() as u64;
        }
    }

    Ok(CountResult {
        file: label,
        reads,
        bases,
    })
}

impl Tabular for CountResult {
    fn columns(&self) -> Vec<Column> {
        vec![
            Column::left("file"),
            Column::right("reads"),
            Column::right("bases"),
        ]
    }

    fn cells(&self) -> Vec<Cell> {
        vec![
            Cell::Text(self.file.clone()),
            Cell::Int(self.reads),
            Cell::Int(self.bases),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(inputs: &[&str], combine: bool, basename: bool) -> CountArgs {
        CountArgs {
            inputs: inputs.iter().map(PathBuf::from).collect(),
            json: false,
            tsv: false,
            combine,
            basename,
        }
    }

    #[test]
    fn per_file_one_row_each() {
        let a = args(
            &["tests/data/small.fastq", "tests/data/small.fasta"],
            false,
            false,
        );
        let rows = compute(&a.inputs, a.combine, a.basename).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].reads, 5);
        assert_eq!(rows[1].reads, 3);
    }

    #[test]
    fn combine_sums_all() {
        let a = args(
            &["tests/data/small.fastq", "tests/data/small.fasta"],
            true,
            false,
        );
        let rows = compute(&a.inputs, a.combine, a.basename).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].reads, 8);
        assert_eq!(rows[0].file, "total");
    }
}
