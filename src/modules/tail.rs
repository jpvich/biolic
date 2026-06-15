//! `biolic tail`: extract the last N reads (or last N bases).
//!
//! Unlike `head`, `tail` must see the end of the stream to know which records
//! are last, so it buffers a **bounded suffix** — never the whole file. Memory
//! is O(N) records for `-n N`, or O(target bases) for `--bases T` (the smallest
//! suffix whose total reaches the target, plus one record). Output preserves the
//! input format, reusing `io::writer`.
//!
//! `--bases` works at whole-record granularity (it never truncates a sequence),
//! matching `seqkit tail`.

use std::collections::VecDeque;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;

use crate::cli::RunContext;
use crate::io::reader::open_input;
use crate::io::{FastaWriter, FastqWriter, Format, RecordWriter};
use crate::output::fmt_commas;
use crate::record::Record;
use crate::utils::size::parse_size;

#[derive(Args, Debug)]
pub struct TailArgs {
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

/// The resolved budget for `tail`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Limit {
    /// Keep the last this-many records.
    Reads(u64),
    /// Keep the smallest trailing set of records whose total reaches this many bases.
    Bases(u64),
}

impl Limit {
    /// `--bases` wins when present; otherwise `-n` (default 10).
    fn from_args(args: &TailArgs) -> Result<Self> {
        match &args.bases {
            Some(s) => Ok(Limit::Bases(parse_size(s)?)),
            None => Ok(Limit::Reads(args.reads.unwrap_or(10))),
        }
    }
}

/// The pure, I/O-free core: a bounded buffer that keeps only the trailing
/// records the limit asks for. `run` feeds it every record via [`offer`] and
/// then takes the survivors with [`into_records`]. Mirrors `sample`'s
/// `Reservoir`. Memory stays bounded by the limit, never the file size.
///
/// [`offer`]: Tail::offer
/// [`into_records`]: Tail::into_records
pub struct Tail {
    limit: Limit,
    buf: VecDeque<Record>,
    /// Total bases currently held in `buf`.
    bases: u64,
}

impl Tail {
    pub fn new(limit: Limit) -> Self {
        Self {
            limit,
            buf: VecDeque::new(),
            bases: 0,
        }
    }

    /// Offer the next record from the stream. Evicts from the front as needed so
    /// the buffer holds exactly the trailing window the limit requires.
    pub fn offer(&mut self, rec: Record) {
        self.bases += rec.len() as u64;
        self.buf.push_back(rec);
        match self.limit {
            Limit::Reads(n) => {
                while self.buf.len() as u64 > n {
                    self.pop_front();
                }
            }
            Limit::Bases(target) => {
                // Keep the minimal suffix whose total reaches `target`: drop the
                // front while doing so would still leave us at or above target.
                while let Some(front) = self.buf.front() {
                    if self.bases - front.len() as u64 >= target {
                        self.pop_front();
                    } else {
                        break;
                    }
                }
            }
        }
    }

    fn pop_front(&mut self) {
        if let Some(front) = self.buf.pop_front() {
            self.bases -= front.len() as u64;
        }
    }

    /// Consume the buffer, yielding the kept records in original order.
    pub fn into_records(self) -> Vec<Record> {
        self.buf.into()
    }
}

pub fn run(args: TailArgs, ctx: &RunContext) -> Result<()> {
    let limit = Limit::from_args(&args)?;

    let mut reader = open_input(&args.input)
        .with_context(|| format!("opening input {}", args.input.display()))?;
    let format = reader.format();

    let mut tail = Tail::new(limit);
    while let Some(rec) = reader.next_record().context("reading record")? {
        tail.offer(rec);
    }
    let records = tail.into_records();

    let stdout = std::io::stdout();
    let handle = std::io::BufWriter::new(stdout.lock());
    let mut writer: Box<dyn RecordWriter> = match format {
        Format::Fasta => Box::new(FastaWriter::new(handle)),
        Format::Fastq | Format::Bam => Box::new(FastqWriter::new(handle)),
    };
    for rec in &records {
        writer.write_record(rec)?;
    }
    writer.flush()?;

    if !ctx.quiet {
        eprintln!(
            "[biolic tail] wrote {} records",
            fmt_commas(records.len() as u64)
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(id: u8, len: usize) -> Record {
        Record::new(
            vec![b'r', b'0' + id],
            vec![b'A'; len],
            Some(vec![b'I'; len]),
        )
    }

    fn ids(records: &[Record]) -> Vec<u8> {
        records.iter().map(|r| r.id[1]).collect()
    }

    #[test]
    fn reads_keeps_last_n_in_order() {
        let mut t = Tail::new(Limit::Reads(2));
        for i in 0..5 {
            t.offer(rec(i, 4));
        }
        let out = t.into_records();
        assert_eq!(out.len(), 2);
        assert_eq!(ids(&out), vec![b'3', b'4']);
    }

    #[test]
    fn reads_more_than_stream_keeps_all() {
        let mut t = Tail::new(Limit::Reads(10));
        for i in 0..3 {
            t.offer(rec(i, 4));
        }
        assert_eq!(t.into_records().len(), 3);
    }

    #[test]
    fn reads_zero_keeps_none() {
        let mut t = Tail::new(Limit::Reads(0));
        for i in 0..3 {
            t.offer(rec(i, 4));
        }
        assert!(t.into_records().is_empty());
    }

    #[test]
    fn bases_keeps_minimal_trailing_suffix() {
        // Five 4-base records (20 bases). Target 10 → last 3 records (12 bases):
        // dropping more would fall below 10.
        let mut t = Tail::new(Limit::Bases(10));
        for i in 0..5 {
            t.offer(rec(i, 4));
        }
        let out = t.into_records();
        assert_eq!(ids(&out), vec![b'2', b'3', b'4']);
    }

    #[test]
    fn bases_target_above_total_keeps_all() {
        let mut t = Tail::new(Limit::Bases(1000));
        for i in 0..3 {
            t.offer(rec(i, 4));
        }
        assert_eq!(t.into_records().len(), 3);
    }
}
