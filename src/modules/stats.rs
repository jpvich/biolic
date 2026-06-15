//! `biolic stats`: compute summary statistics for sequence files.
//!
//! Fully implemented for FASTQ, FASTA, and unaligned BAM input. It demonstrates
//! the patterns used by other modules:
//! - Define `Args` struct with `clap::Args` derive
//! - Define `run()` entry point taking `(Args, &RunContext)`
//! - Define a result type that implements `Serialize` and `Tabular`
//! - Use streaming reader from `io::reader`
//! - Output via `output::write_rows()` respecting the user's format choice.
//!
//! By default each input file produces its own row (so samples can be compared,
//! like `seqkit stats`). `--combine` aggregates every input into one row.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;

use crate::cli::RunContext;
use crate::io::reader::open_input;
use crate::io::Format;
use crate::output::{write_rows, Cell, Column, OutputFormat, Tabular};
use crate::utils::quality::{count_above_threshold, mean_quality, phred_to_prob, prob_to_phred};
use crate::utils::stats_helpers::{mean, median, min_max, nx, percentile, percentile_f64};

#[derive(Args, Debug)]
pub struct StatsArgs {
    /// Input files (FASTQ, FASTA, or unaligned BAM; optionally gzipped).
    /// Each file is summarized on its own row. With no file, reads stdin.
    pub inputs: Vec<PathBuf>,

    /// Output in JSON format.
    #[arg(long, conflicts_with = "tsv")]
    pub json: bool,

    /// Output in TSV format.
    #[arg(long, conflicts_with = "json")]
    pub tsv: bool,

    /// Print extended statistics (length and quality percentile distributions).
    #[arg(short = 'e', long)]
    pub extended: bool,

    /// Aggregate all inputs into a single combined row instead of one per file.
    #[arg(long)]
    pub combine: bool,

    /// Show only the file's basename in the `file` column.
    #[arg(short = 'b', long)]
    pub basename: bool,
}

/// Length/quality distribution percentiles (populated only with `--extended`).
#[derive(Debug, Serialize)]
pub struct Percentiles {
    pub p10: f64,
    pub p25: f64,
    pub p50: f64,
    pub p75: f64,
    pub p90: f64,
    pub p99: f64,
}

impl Percentiles {
    fn from_u64(values: &[u64]) -> Self {
        Self {
            p10: percentile(values, 10.0),
            p25: percentile(values, 25.0),
            p50: percentile(values, 50.0),
            p75: percentile(values, 75.0),
            p90: percentile(values, 90.0),
            p99: percentile(values, 99.0),
        }
    }

    fn from_f64(values: &[f64]) -> Self {
        Self {
            p10: percentile_f64(values, 10.0),
            p25: percentile_f64(values, 25.0),
            p50: percentile_f64(values, 50.0),
            p75: percentile_f64(values, 75.0),
            p90: percentile_f64(values, 90.0),
            p99: percentile_f64(values, 99.0),
        }
    }
}

/// Computed statistics for one row (a single file, or the combined total).
#[derive(Debug, Serialize)]
pub struct Stats {
    pub file: String,
    /// Detected input format (FASTQ/FASTA/BAM, or "mixed" under `--combine`).
    pub format: String,
    /// Detected sequence alphabet (DNA/RNA/Protein).
    pub seq_type: String,
    pub read_count: u64,
    pub total_bases: u64,
    pub min_length: u64,
    pub max_length: u64,
    pub mean_length: f64,
    pub median_length: f64,
    pub n50: u64,
    pub n90: u64,
    pub mean_quality: f64,
    pub bases_above_q10_pct: f64,
    pub bases_above_q20_pct: f64,
    pub bases_above_q30_pct: f64,
    pub gc_content_pct: f64,
    /// Count of ambiguous bases (N/n). Shown as `sum_n`, matching `seqkit`.
    pub ambiguous_bases: u64,

    /// Length distribution percentiles (only with `--extended`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length_percentiles: Option<Percentiles>,

    /// Per-read mean-quality percentiles (only with `--extended`, and only when
    /// the input carries quality scores).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality_percentiles: Option<Percentiles>,

    /// Whether extended columns should be shown in the table. Not serialized;
    /// it only governs column layout so every row in a run is consistent.
    #[serde(skip)]
    extended: bool,
}

pub fn run(args: StatsArgs, ctx: &RunContext) -> Result<()> {
    let inputs = resolved_inputs(&args.inputs);
    if ctx.verbose {
        eprintln!(
            "[biolic stats] Reading from {}",
            inputs
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let rows = compute(&args.inputs, args.extended, args.combine, args.basename)?;
    let format = OutputFormat::from_flags(args.json, args.tsv);
    write_rows(&rows, format)?;
    Ok(())
}

/// Resolve the input list: no files means read from stdin.
fn resolved_inputs(inputs: &[PathBuf]) -> Vec<PathBuf> {
    if inputs.is_empty() {
        vec![PathBuf::from("-")]
    } else {
        inputs.to_vec()
    }
}

/// Label shown in the `file` column for one input path.
fn label_for(p: &Path, basename: bool) -> String {
    if basename {
        p.file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| p.display().to_string())
    } else {
        p.display().to_string()
    }
}

/// Build the output rows: one per input file, or a single combined row.
///
/// Pure core (no `clap`, no I/O beyond reading the inputs): future Python
/// bindings call this directly. `run` is the thin CLI wrapper around it.
pub fn compute(
    inputs: &[PathBuf],
    extended: bool,
    combine: bool,
    basename: bool,
) -> Result<Vec<Stats>> {
    let inputs = resolved_inputs(inputs);

    if combine {
        let label = "total".to_string();
        Ok(vec![aggregate(&inputs, extended, label)?])
    } else {
        inputs
            .iter()
            .map(|input| {
                let label = label_for(input, basename);
                aggregate(std::slice::from_ref(input), extended, label)
            })
            .collect()
    }
}

/// Aggregate statistics over a set of inputs into a single [`Stats`] row.
///
/// Memory: O(N) in the number of reads (a Vec of lengths is kept for exact N50,
/// and a Vec of per-read mean qualities when `extended` is set). For
/// terabyte-scale files a histogram approximation could be added later.
fn aggregate(inputs: &[PathBuf], extended: bool, label: String) -> Result<Stats> {
    let mut read_count: u64 = 0;
    let mut total_bases: u64 = 0;
    let mut lengths: Vec<u64> = Vec::new();

    // Aggregate quality across all reads using probability-space averaging.
    let mut sum_error_prob: f64 = 0.0;
    let mut bases_q10: u64 = 0;
    let mut bases_q20: u64 = 0;
    let mut bases_q30: u64 = 0;

    // Aggregate base composition. `acgt_count` is the GC denominator and counts
    // A/C/G/T plus U (RNA), so GC% is correct for both DNA and RNA.
    let mut gc_count: u64 = 0;
    let mut acgt_count: u64 = 0;
    let mut ambiguous_bases: u64 = 0;

    // Sequence-alphabet signals used to classify the input as DNA/RNA/Protein.
    let mut saw_u = false;
    let mut saw_t = false;
    let mut saw_protein = false;

    // Distinct input formats seen (one per file); used for the `format` column.
    let mut formats: Vec<Format> = Vec::new();

    // Per-read mean qualities, only collected for extended output.
    let mut read_mean_quals: Vec<f64> = Vec::new();

    for input in inputs {
        let mut reader =
            open_input(input).with_context(|| format!("opening input {}", input.display()))?;
        let fmt = reader.format();
        if !formats.contains(&fmt) {
            formats.push(fmt);
        }

        while let Some(record) = reader.next_record().context("reading record")? {
            read_count += 1;
            total_bases += record.len() as u64;
            lengths.push(record.len() as u64);

            // Quality stats only if quality scores are present.
            if let Some(qual) = &record.qual {
                for &q in qual {
                    sum_error_prob += phred_to_prob(q);
                }
                bases_q10 += count_above_threshold(qual, 10) as u64;
                bases_q20 += count_above_threshold(qual, 20) as u64;
                bases_q30 += count_above_threshold(qual, 30) as u64;

                if extended {
                    read_mean_quals.push(mean_quality(qual));
                }
            }

            // Single pass over the sequence: base composition, ambiguous count,
            // and alphabet detection (U for RNA, protein-only residues).
            for &b in &record.seq {
                match b {
                    b'A' | b'a' => acgt_count += 1,
                    b'C' | b'c' | b'G' | b'g' => {
                        acgt_count += 1;
                        gc_count += 1;
                    }
                    b'T' | b't' => {
                        acgt_count += 1;
                        saw_t = true;
                    }
                    b'U' | b'u' => {
                        acgt_count += 1;
                        saw_u = true;
                    }
                    b'N' | b'n' => ambiguous_bases += 1,
                    // Residues that appear only in protein, never in IUPAC
                    // nucleotide codes (A C G T U R Y S W K M B D H V N).
                    b'E' | b'e' | b'F' | b'f' | b'I' | b'i' | b'J' | b'j' | b'L' | b'l' | b'O'
                    | b'o' | b'P' | b'p' | b'Q' | b'q' | b'Z' | b'z' => saw_protein = true,
                    _ => {}
                }
            }
        }
    }

    let (min_length, max_length) = min_max(&lengths);
    let mean_length = mean(&lengths);
    let median_length = median(&lengths);
    let n50_val = nx(&lengths, 50);
    let n90_val = nx(&lengths, 90);

    let mean_qual = if total_bases > 0 && sum_error_prob > 0.0 {
        prob_to_phred(sum_error_prob / total_bases as f64)
    } else {
        0.0
    };

    let length_percentiles = if extended {
        Some(Percentiles::from_u64(&lengths))
    } else {
        None
    };
    let quality_percentiles = if extended && !read_mean_quals.is_empty() {
        Some(Percentiles::from_f64(&read_mean_quals))
    } else {
        None
    };

    let format = match formats.as_slice() {
        [one] => one.name().to_string(),
        [] => "-".to_string(),
        _ => "mixed".to_string(),
    };
    let seq_type = classify_type(saw_u, saw_t, saw_protein).to_string();

    Ok(Stats {
        file: label,
        format,
        seq_type,
        read_count,
        total_bases,
        min_length,
        max_length,
        mean_length,
        median_length,
        n50: n50_val,
        n90: n90_val,
        mean_quality: mean_qual,
        bases_above_q10_pct: pct(bases_q10, total_bases),
        bases_above_q20_pct: pct(bases_q20, total_bases),
        bases_above_q30_pct: pct(bases_q30, total_bases),
        gc_content_pct: pct(gc_count, acgt_count),
        ambiguous_bases,
        length_percentiles,
        quality_percentiles,
        extended,
    })
}

fn pct(num: u64, denom: u64) -> f64 {
    if denom == 0 {
        0.0
    } else {
        100.0 * (num as f64) / (denom as f64)
    }
}

/// Classify the sequence alphabet from the signals gathered while scanning.
/// Protein wins if any protein-only residue was seen; otherwise RNA if U is
/// present without T; otherwise DNA (the default, including empty input).
fn classify_type(saw_u: bool, saw_t: bool, saw_protein: bool) -> &'static str {
    if saw_protein {
        "Protein"
    } else if saw_u && !saw_t {
        "RNA"
    } else {
        "DNA"
    }
}

impl Tabular for Stats {
    fn columns(&self) -> Vec<Column> {
        let mut cols = vec![
            Column::left("file"),
            Column::left("format"),
            Column::left("type"),
            Column::right("reads"),
            Column::right("total_bases"),
            Column::right("min_length"),
            Column::right("max_length"),
            Column::right("mean_length"),
            Column::right("median_length"),
            Column::right("n50"),
            Column::right("n90"),
            Column::right("mean_quality"),
            Column::right("q10_pct"),
            Column::right("q20_pct"),
            Column::right("q30_pct"),
            Column::right("gc_pct"),
            Column::right("sum_n"),
        ];
        if self.extended {
            cols.extend([
                Column::right("len_p10"),
                Column::right("len_p25"),
                Column::right("len_p50"),
                Column::right("len_p75"),
                Column::right("len_p90"),
                Column::right("len_p99"),
                Column::right("qual_p10"),
                Column::right("qual_p25"),
                Column::right("qual_p50"),
                Column::right("qual_p75"),
                Column::right("qual_p90"),
                Column::right("qual_p99"),
            ]);
        }
        cols
    }

    fn cells(&self) -> Vec<Cell> {
        let mut cells = vec![
            Cell::Text(self.file.clone()),
            Cell::Text(self.format.clone()),
            Cell::Text(self.seq_type.clone()),
            Cell::Int(self.read_count),
            Cell::Int(self.total_bases),
            Cell::Int(self.min_length),
            Cell::Int(self.max_length),
            Cell::Float(self.mean_length, 1),
            Cell::Float(self.median_length, 1),
            Cell::Int(self.n50),
            Cell::Int(self.n90),
            Cell::Float(self.mean_quality, 2),
            Cell::Percent(self.bases_above_q10_pct),
            Cell::Percent(self.bases_above_q20_pct),
            Cell::Percent(self.bases_above_q30_pct),
            Cell::Percent(self.gc_content_pct),
            Cell::Int(self.ambiguous_bases),
        ];
        if self.extended {
            // Length percentiles are always present under --extended.
            let lp = self.length_percentiles.as_ref();
            for v in [
                lp.map(|p| p.p10),
                lp.map(|p| p.p25),
                lp.map(|p| p.p50),
                lp.map(|p| p.p75),
                lp.map(|p| p.p90),
                lp.map(|p| p.p99),
            ] {
                cells.push(percentile_cell(v, 1));
            }
            // Quality percentiles are absent for inputs without quality (FASTA).
            let qp = self.quality_percentiles.as_ref();
            for v in [
                qp.map(|p| p.p10),
                qp.map(|p| p.p25),
                qp.map(|p| p.p50),
                qp.map(|p| p.p75),
                qp.map(|p| p.p90),
                qp.map(|p| p.p99),
            ] {
                cells.push(percentile_cell(v, 2));
            }
        }
        cells
    }
}

/// A percentile cell: the value, or a dash when not applicable (e.g. quality
/// percentiles for a FASTA file that has no quality scores).
fn percentile_cell(v: Option<f64>, precision: usize) -> Cell {
    match v {
        Some(x) => Cell::Float(x, precision),
        None => Cell::Text("-".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(inputs: &[&str], combine: bool, extended: bool, basename: bool) -> StatsArgs {
        StatsArgs {
            inputs: inputs.iter().map(PathBuf::from).collect(),
            json: false,
            tsv: false,
            extended,
            combine,
            basename,
        }
    }

    #[test]
    fn per_file_produces_one_row_each() {
        let a = args(
            &["tests/data/small.fastq", "tests/data/small.fasta"],
            false,
            false,
            false,
        );
        let rows = compute(&a.inputs, a.extended, a.combine, a.basename).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].read_count, 5); // small.fastq
        assert_eq!(rows[1].read_count, 3); // small.fasta
        assert_eq!(rows[0].format, "FASTQ");
        assert_eq!(rows[1].format, "FASTA");
        assert_eq!(rows[0].seq_type, "DNA");
        assert_eq!(rows[1].seq_type, "DNA");
    }

    #[test]
    fn combine_mixed_formats_reports_mixed() {
        let a = args(
            &["tests/data/small.fastq", "tests/data/small.fasta"],
            true,
            false,
            false,
        );
        let rows = compute(&a.inputs, a.extended, a.combine, a.basename).unwrap();
        assert_eq!(rows[0].format, "mixed");
    }

    #[test]
    fn combine_aggregates_into_one_row() {
        let a = args(
            &["tests/data/small.fastq", "tests/data/small.fasta"],
            true,
            false,
            false,
        );
        let rows = compute(&a.inputs, a.extended, a.combine, a.basename).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].read_count, 8); // 5 + 3
        assert_eq!(rows[0].file, "total");
    }

    #[test]
    fn basename_strips_directory() {
        let a = args(&["tests/data/small.fastq"], false, false, true);
        let rows = compute(&a.inputs, a.extended, a.combine, a.basename).unwrap();
        assert_eq!(rows[0].file, "small.fastq");
    }
}
