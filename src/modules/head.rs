//! `biolic head`: extract the first N reads (or first N bases).
//!
//! Streaming and ideal: `head` stops reading as soon as the budget is met, so it
//! never touches the rest of the file. Output preserves the input format
//! (FASTA stays FASTA; FASTQ/BAM are written as FASTQ), reusing `io::writer`.
//!
//! `--bases` works at whole-record granularity (it never truncates a sequence):
//! records are written until the cumulative base count reaches the target, so
//! the output may slightly overshoot. This matches `seqkit head`.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;

use crate::cli::RunContext;
use crate::io::reader::open_input;
use crate::io::{FastaWriter, FastqWriter, Format, RecordWriter};
use crate::output::fmt_commas;
use crate::utils::size::parse_size;

#[derive(Args, Debug)]
pub struct HeadArgs {
    /// Input file (FASTQ/FASTA/BAM, optionally gzipped). "-" or omit for stdin.
    #[arg(default_value = "-")]
    pub input: PathBuf,

    /// Number of reads to output (default 10). Mutually exclusive with --bases.
    #[arg(short = 'n', long, conflicts_with = "bases")]
    pub reads: Option<u64>,

    /// Number of bases to output (K/M/G suffixes, e.g. 100M). Whole records only,
    /// so the output may slightly overshoot the target.
    #[arg(long, value_name = "SIZE")]
    pub bases: Option<String>,
}

/// The resolved budget. This is the pure, I/O-free core: [`Limit::reached`]
/// decides when to stop, and `run` only drives the read/write loop around it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    /// Stop after this many records.
    Reads(u64),
    /// Stop once the cumulative base count reaches this target.
    Bases(u64),
}

impl Limit {
    /// `--bases` wins when present; otherwise `-n` (default 10).
    fn from_args(args: &HeadArgs) -> Result<Self> {
        match &args.bases {
            Some(s) => Ok(Limit::Bases(parse_size(s)?)),
            None => Ok(Limit::Reads(args.reads.unwrap_or(10))),
        }
    }

    /// Whether the budget is already met *before* pulling another record.
    /// For `Bases`, this returns true only once the previously written records
    /// have reached the target, so the record that crosses the line is still
    /// written (whole-record granularity).
    pub fn reached(&self, written: u64, bases: u64) -> bool {
        match self {
            Limit::Reads(n) => written >= *n,
            Limit::Bases(b) => bases >= *b,
        }
    }
}

pub fn run(args: HeadArgs, ctx: &RunContext) -> Result<()> {
    let limit = Limit::from_args(&args)?;

    let mut reader = open_input(&args.input)
        .with_context(|| format!("opening input {}", args.input.display()))?;

    let stdout = std::io::stdout();
    let handle = std::io::BufWriter::new(stdout.lock());
    let mut writer: Box<dyn RecordWriter> = match reader.format() {
        Format::Fasta => Box::new(FastaWriter::new(handle)),
        Format::Fastq | Format::Bam => Box::new(FastqWriter::new(handle)),
    };

    let mut written = 0u64;
    let mut bases = 0u64;
    while !limit.reached(written, bases) {
        match reader.next_record().context("reading record")? {
            Some(rec) => {
                writer.write_record(&rec)?;
                written += 1;
                bases += rec.len() as u64;
            }
            None => break,
        }
    }
    writer.flush()?;

    if !ctx.quiet {
        eprintln!("[biolic head] wrote {} records", fmt_commas(written));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_limit_stops_at_count() {
        let l = Limit::Reads(2);
        assert!(!l.reached(0, 0));
        assert!(!l.reached(1, 0));
        assert!(l.reached(2, 0));
        assert!(l.reached(3, 0));
    }

    #[test]
    fn reads_limit_zero_writes_nothing() {
        assert!(Limit::Reads(0).reached(0, 0));
    }

    #[test]
    fn bases_limit_stops_when_target_reached() {
        let l = Limit::Bases(100);
        assert!(!l.reached(1, 99)); // one short → write the crossing record
        assert!(l.reached(1, 100)); // exactly met
        assert!(l.reached(2, 150)); // overshoot is fine
    }

    #[test]
    fn from_args_defaults_to_ten_reads() {
        let a = HeadArgs {
            input: PathBuf::from("-"),
            reads: None,
            bases: None,
        };
        assert_eq!(Limit::from_args(&a).unwrap(), Limit::Reads(10));
    }

    #[test]
    fn from_args_bases_overrides() {
        let a = HeadArgs {
            input: PathBuf::from("-"),
            reads: None,
            bases: Some("1K".to_string()),
        };
        assert_eq!(Limit::from_args(&a).unwrap(), Limit::Bases(1_000));
    }
}
