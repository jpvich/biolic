//! `biolic filter`: keep or drop reads by length, quality, GC, and N content,
//! with optional end trimming (fixed crops and quality-based best-segment).
//!
//! Like `nanoq` and `chopper`, filtering and trimming live in one command: a
//! read is first trimmed (fixed crops, then quality trim), then evaluated
//! against the filters. Streaming: each read is handled independently in O(1)
//! memory. Output goes to stdout in the input's format (FASTQ/BAM → FASTQ,
//! FASTA → FASTA); a kept/total summary goes to stderr.
//!
//! The decision logic lives in [`Filter`], a pure core with no I/O: build it,
//! then call [`Filter::apply`] per record. `run` only wires reader → core →
//! writer. (See the Module Contract in `ARCHITECTURE.md`.)

use std::io::BufWriter;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;

use crate::cli::RunContext;
use crate::io::reader::open_input;
use crate::io::{FastaWriter, FastqWriter, Format, RecordWriter};
use crate::output::fmt_commas;
use crate::record::Record;
use crate::utils::quality::{mean_quality, phred_to_prob};

#[derive(Args, Debug)]
pub struct FilterArgs {
    /// Input file (FASTQ, FASTA, or unaligned BAM; optionally gzipped).
    /// Use "-" or omit for stdin.
    #[arg(default_value = "-")]
    pub input: PathBuf,

    /// Minimum read length to keep.
    #[arg(short = 'l', long)]
    pub min_length: Option<u64>,

    /// Maximum read length to keep.
    #[arg(short = 'm', long)]
    pub max_length: Option<u64>,

    /// Minimum mean quality (Phred) to keep. Ignored for inputs without quality.
    #[arg(short = 'q', long)]
    pub min_quality: Option<f64>,

    /// Maximum proportion of N bases allowed (0.0-1.0).
    #[arg(long)]
    pub max_n: Option<f64>,

    /// Minimum GC content (0.0-1.0).
    #[arg(long)]
    pub min_gc: Option<f64>,

    /// Maximum GC content (0.0-1.0).
    #[arg(long)]
    pub max_gc: Option<f64>,

    /// Remove N bases from the start of each read (applied before filtering).
    #[arg(long)]
    pub headcrop: Option<usize>,

    /// Remove N bases from the end of each read (applied before filtering).
    #[arg(long)]
    pub tailcrop: Option<usize>,

    /// Quality-trim both ends to the highest-quality segment, using this Phred
    /// cutoff (Mott's algorithm). Ignored for inputs without quality.
    #[arg(long, value_name = "PHRED")]
    pub trim_quality: Option<f64>,
}

pub fn run(args: FilterArgs, ctx: &RunContext) -> Result<()> {
    let mut reader = open_input(&args.input)
        .with_context(|| format!("opening input {}", args.input.display()))?;
    let format = reader.format();

    let stdout = std::io::stdout();
    let handle = BufWriter::new(stdout.lock());
    let mut writer: Box<dyn RecordWriter> = match format {
        Format::Fasta => Box::new(FastaWriter::new(handle)),
        Format::Fastq | Format::Bam => Box::new(FastqWriter::new(handle)),
    };

    // The only I/O loop; all decisions are in the pure `Filter` core.
    let filter = Filter::from_args(&args);
    let mut total = 0u64;
    let mut kept = 0u64;
    while let Some(rec) = reader.next_record().context("reading record")? {
        total += 1;
        if let Some(out) = filter.apply(rec) {
            writer.write_record(&out)?;
            kept += 1;
        }
    }
    writer.flush()?;

    if !ctx.quiet {
        let pct = if total == 0 {
            0.0
        } else {
            100.0 * kept as f64 / total as f64
        };
        eprintln!(
            "[biolic filter] kept {} / {} reads ({:.1}%)",
            fmt_commas(kept),
            fmt_commas(total),
            pct
        );
    }
    Ok(())
}

/// The pure filter + trim core, independent of any input/output.
///
/// Build it from CLI args (or construct it directly), then call
/// [`apply`](Filter::apply) on each record. Holds plain fields — no dependency
/// on clap or the CLI — so future Python bindings can wrap this same type.
#[derive(Debug, Clone)]
pub struct Filter {
    pub min_length: Option<u64>,
    pub max_length: Option<u64>,
    pub min_quality: Option<f64>,
    pub max_n: Option<f64>,
    pub min_gc: Option<f64>,
    pub max_gc: Option<f64>,
    pub headcrop: usize,
    pub tailcrop: usize,
    pub trim_quality: Option<f64>,
}

impl Filter {
    fn from_args(a: &FilterArgs) -> Self {
        Self {
            min_length: a.min_length,
            max_length: a.max_length,
            min_quality: a.min_quality,
            max_n: a.max_n,
            min_gc: a.min_gc,
            max_gc: a.max_gc,
            headcrop: a.headcrop.unwrap_or(0),
            tailcrop: a.tailcrop.unwrap_or(0),
            trim_quality: a.trim_quality,
        }
    }

    /// Trim a record, then test it. Returns the kept (possibly trimmed) record,
    /// or `None` if it is trimmed away or fails a filter.
    pub fn apply(&self, mut rec: Record) -> Option<Record> {
        self.trim(&mut rec);
        // A read trimmed away entirely cannot pass any length filter.
        if rec.seq.is_empty() {
            return None;
        }
        if self.passes(&rec) {
            Some(rec)
        } else {
            None
        }
    }

    /// Apply trimming in place: fixed head/tail crops first, then quality trim.
    fn trim(&self, rec: &mut Record) {
        let (head, tail) = (self.headcrop, self.tailcrop);
        if head > 0 || tail > 0 {
            if head + tail >= rec.seq.len() {
                rec.seq.clear();
                if let Some(q) = rec.qual.as_mut() {
                    q.clear();
                }
                return;
            }
            let end = rec.seq.len() - tail;
            rec.seq = rec.seq[head..end].to_vec();
            if let Some(q) = rec.qual.as_mut() {
                *q = q[head..end].to_vec();
            }
        }

        if let Some(cutoff) = self.trim_quality {
            // Compute the segment from quality first to avoid borrow conflicts.
            let seg = rec.qual.as_ref().map(|q| best_segment(q, cutoff));
            if let Some(seg) = seg {
                match seg {
                    Some((s, e)) => {
                        rec.seq = rec.seq[s..e].to_vec();
                        if let Some(q) = rec.qual.take() {
                            rec.qual = Some(q[s..e].to_vec());
                        }
                    }
                    None => {
                        rec.seq.clear();
                        rec.qual = Some(Vec::new());
                    }
                }
            }
        }
    }

    /// Evaluate a (possibly trimmed) record against all filters (logical AND).
    fn passes(&self, rec: &Record) -> bool {
        let len = rec.len() as u64;
        if let Some(min) = self.min_length {
            if len < min {
                return false;
            }
        }
        if let Some(max) = self.max_length {
            if len > max {
                return false;
            }
        }
        // Quality is only meaningful when the record carries scores (not FASTA).
        if let Some(min_q) = self.min_quality {
            if let Some(q) = &rec.qual {
                if mean_quality(q) < min_q {
                    return false;
                }
            }
        }
        if self.min_gc.is_some() || self.max_gc.is_some() {
            let gc = rec.gc_content();
            if let Some(min) = self.min_gc {
                if gc < min {
                    return false;
                }
            }
            if let Some(max) = self.max_gc {
                if gc > max {
                    return false;
                }
            }
        }
        if let Some(max_n) = self.max_n {
            let (_, _, _, _, n, _) = rec.base_counts();
            let prop = if len == 0 { 0.0 } else { n as f64 / len as f64 };
            if prop > max_n {
                return false;
            }
        }
        true
    }
}

/// Mott's algorithm: find the contiguous segment maximizing the cumulative
/// `(cutoff_error_prob - base_error_prob)`, i.e. the highest-quality stretch.
/// Returns `[start, end)` indices, or `None` if no base clears the cutoff.
fn best_segment(qual: &[u8], cutoff_phred: f64) -> Option<(usize, usize)> {
    let cutoff_prob = 10f64.powf(-cutoff_phred / 10.0);
    let mut best: Option<(usize, usize)> = None;
    let mut best_err = 0.0f64;
    let mut best_len = 0usize;
    let mut cur_start = 0usize;
    let mut cur_err = -1.0f64;
    for (i, &c) in qual.iter().enumerate() {
        let contribution = cutoff_prob - phred_to_prob(c);
        if cur_err < 0.0 {
            cur_err = 0.0;
            cur_start = i;
        }
        cur_err += contribution;
        let cur_len = i - cur_start + 1;
        if best.is_none() || cur_err > best_err || (cur_err == best_err && cur_len > best_len) {
            best = Some((cur_start, i + 1));
            best_err = cur_err;
            best_len = cur_len;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> FilterArgs {
        FilterArgs {
            input: PathBuf::from("-"),
            min_length: None,
            max_length: None,
            min_quality: None,
            max_n: None,
            min_gc: None,
            max_gc: None,
            headcrop: None,
            tailcrop: None,
            trim_quality: None,
        }
    }

    fn rec(seq: &[u8], qual: &[u8]) -> Record {
        Record::new(b"r".to_vec(), seq.to_vec(), Some(qual.to_vec()))
    }

    #[test]
    fn min_length_drops_short_reads() {
        let f = Filter::from_args(&FilterArgs {
            min_length: Some(4),
            ..args()
        });
        assert!(f.apply(rec(b"AC", b"II")).is_none());
        assert!(f.apply(rec(b"ACGT", b"IIII")).is_some());
    }

    #[test]
    fn min_quality_uses_probability_space() {
        let f = Filter::from_args(&FilterArgs {
            min_quality: Some(15.0),
            ..args()
        });
        assert!(f.apply(rec(b"ACGT", b"IIII")).is_some()); // 'I' = Q40
        assert!(f.apply(rec(b"ACGT", b"++++")).is_none()); // '+' = Q10
    }

    #[test]
    fn quality_filter_ignored_for_fasta() {
        let f = Filter::from_args(&FilterArgs {
            min_quality: Some(30.0),
            ..args()
        });
        // No quality (FASTA-like): quality filter cannot apply, so it passes.
        let r = Record::new(b"r".to_vec(), b"ACGT".to_vec(), None);
        assert!(f.apply(r).is_some());
    }

    #[test]
    fn headcrop_tailcrop_shorten_read() {
        let f = Filter::from_args(&FilterArgs {
            headcrop: Some(1),
            tailcrop: Some(1),
            ..args()
        });
        let out = f.apply(rec(b"AACGTT", b"IIIIII")).unwrap();
        assert_eq!(out.seq, b"ACGT");
        assert_eq!(out.qual.unwrap(), b"IIII");
    }

    #[test]
    fn crop_larger_than_read_drops_it() {
        let f = Filter::from_args(&FilterArgs {
            headcrop: Some(10),
            ..args()
        });
        assert!(f.apply(rec(b"ACGT", b"IIII")).is_none());
    }

    #[test]
    fn best_segment_keeps_high_quality_middle() {
        // Low-quality ends ('!' = Q0), high-quality middle ('I' = Q40).
        let qual = b"!!IIII!!";
        let seg = best_segment(qual, 20.0).unwrap();
        assert_eq!(seg, (2, 6));
    }

    #[test]
    fn max_n_filter_drops_ambiguous_reads() {
        let f = Filter::from_args(&FilterArgs {
            max_n: Some(0.2),
            ..args()
        });
        // 1 N in 4 bases = 0.25 > 0.20 -> dropped.
        assert!(f.apply(rec(b"ACGN", b"IIII")).is_none());
        assert!(f.apply(rec(b"ACGT", b"IIII")).is_some());
    }
}
