//! `biolic stats`: compute summary statistics for sequence files.
//!
//! Fully implemented for FASTQ, FASTA, and unaligned BAM input. It demonstrates
//! the patterns used by other modules:
//! - Define `Args` struct with `clap::Args` derive
//! - Define `run()` entry point taking `(Args, &RunContext)`
//! - Define a result type that implements `Serialize` and `HumanDisplay`
//! - Use streaming reader from `io::reader`
//! - Output via `output::write()` respecting user's format choice.
//!
//! Multiple input files are treated as one logical stream (plan §7.2): the
//! reported statistics aggregate across all of them.

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;

use crate::cli::RunContext;
use crate::io::reader::open_input;
use crate::output::{write, HumanDisplay, OutputFormat};
use crate::utils::quality::{count_above_threshold, mean_quality, phred_to_prob, prob_to_phred};
use crate::utils::stats_helpers::{mean, median, min_max, nx, percentile, percentile_f64};

#[derive(Args, Debug)]
pub struct StatsArgs {
    /// Input files (FASTQ, FASTA, or unaligned BAM; optionally gzipped).
    /// Multiple files are aggregated as one stream. With no file, reads stdin.
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

/// Computed statistics, aggregated across all inputs.
#[derive(Debug, Serialize)]
pub struct Stats {
    pub file: String,
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

    /// Length distribution percentiles (only with `--extended`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length_percentiles: Option<Percentiles>,

    /// Per-read mean-quality percentiles (only with `--extended`, and only when
    /// the input carries quality scores).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality_percentiles: Option<Percentiles>,
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

    let stats = compute(&args)?;

    let format = if args.json {
        OutputFormat::Json
    } else if args.tsv {
        OutputFormat::Tsv
    } else {
        OutputFormat::auto()
    };

    write(&stats, format)?;
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

/// Core computation: a single streaming pass over every input.
///
/// Memory: O(N) in the number of reads (a Vec of lengths is kept for exact N50,
/// and a Vec of per-read mean qualities when `--extended` is set). For
/// terabyte-scale files a histogram approximation could be added later.
pub fn compute(args: &StatsArgs) -> Result<Stats> {
    let inputs = resolved_inputs(&args.inputs);

    let mut read_count: u64 = 0;
    let mut total_bases: u64 = 0;
    let mut lengths: Vec<u64> = Vec::new();

    // Aggregate quality across all reads using probability-space averaging.
    let mut sum_error_prob: f64 = 0.0;
    let mut bases_q10: u64 = 0;
    let mut bases_q20: u64 = 0;
    let mut bases_q30: u64 = 0;

    // Aggregate base composition.
    let mut gc_count: u64 = 0;
    let mut acgt_count: u64 = 0;

    // Per-read mean qualities, only collected for --extended.
    let mut read_mean_quals: Vec<f64> = Vec::new();

    for input in &inputs {
        let mut reader =
            open_input(input).with_context(|| format!("opening input {}", input.display()))?;

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

                if args.extended {
                    read_mean_quals.push(mean_quality(qual));
                }
            }

            let (a, c, g, t, _n, _other) = record.base_counts();
            acgt_count += a + c + g + t;
            gc_count += c + g;
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

    let length_percentiles = if args.extended {
        Some(Percentiles::from_u64(&lengths))
    } else {
        None
    };
    let quality_percentiles = if args.extended && !read_mean_quals.is_empty() {
        Some(Percentiles::from_f64(&read_mean_quals))
    } else {
        None
    };

    let file = inputs
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(",");

    Ok(Stats {
        file,
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
        length_percentiles,
        quality_percentiles,
    })
}

fn pct(num: u64, denom: u64) -> f64 {
    if denom == 0 {
        0.0
    } else {
        100.0 * (num as f64) / (denom as f64)
    }
}

impl HumanDisplay for Stats {
    fn write_human(&self, w: &mut dyn std::io::Write) -> Result<()> {
        writeln!(w, "File:              {}", self.file)?;
        writeln!(w, "Reads:             {}", fmt_int(self.read_count))?;
        writeln!(w, "Total bases:       {}", fmt_int(self.total_bases))?;
        writeln!(w, "Min length:        {}", fmt_int(self.min_length))?;
        writeln!(w, "Max length:        {}", fmt_int(self.max_length))?;
        writeln!(w, "Mean length:       {:.1}", self.mean_length)?;
        writeln!(w, "Median length:     {:.1}", self.median_length)?;
        writeln!(w, "N50:               {}", fmt_int(self.n50))?;
        writeln!(w, "N90:               {}", fmt_int(self.n90))?;
        writeln!(w, "Mean quality:      {:.2}", self.mean_quality)?;
        writeln!(w, "Bases above Q10:   {:.2}%", self.bases_above_q10_pct)?;
        writeln!(w, "Bases above Q20:   {:.2}%", self.bases_above_q20_pct)?;
        writeln!(w, "Bases above Q30:   {:.2}%", self.bases_above_q30_pct)?;
        writeln!(w, "GC content:        {:.2}%", self.gc_content_pct)?;

        if let Some(lp) = &self.length_percentiles {
            writeln!(w, "Length percentiles:")?;
            writeln!(
                w,
                "  P10 {:.0}  P25 {:.0}  P50 {:.0}  P75 {:.0}  P90 {:.0}  P99 {:.0}",
                lp.p10, lp.p25, lp.p50, lp.p75, lp.p90, lp.p99
            )?;
        }
        if let Some(qp) = &self.quality_percentiles {
            writeln!(w, "Quality percentiles (per-read mean):")?;
            writeln!(
                w,
                "  P10 {:.2}  P25 {:.2}  P50 {:.2}  P75 {:.2}  P90 {:.2}  P99 {:.2}",
                qp.p10, qp.p25, qp.p50, qp.p75, qp.p90, qp.p99
            )?;
        }
        Ok(())
    }

    fn write_tsv(&self, w: &mut dyn std::io::Write) -> Result<()> {
        let mut headers: Vec<&str> = vec![
            "file",
            "reads",
            "total_bases",
            "min_length",
            "max_length",
            "mean_length",
            "median_length",
            "n50",
            "n90",
            "mean_quality",
            "q10_pct",
            "q20_pct",
            "q30_pct",
            "gc_pct",
        ];
        let mut values: Vec<String> = vec![
            self.file.clone(),
            self.read_count.to_string(),
            self.total_bases.to_string(),
            self.min_length.to_string(),
            self.max_length.to_string(),
            format!("{:.1}", self.mean_length),
            format!("{:.1}", self.median_length),
            self.n50.to_string(),
            self.n90.to_string(),
            format!("{:.2}", self.mean_quality),
            format!("{:.2}", self.bases_above_q10_pct),
            format!("{:.2}", self.bases_above_q20_pct),
            format!("{:.2}", self.bases_above_q30_pct),
            format!("{:.2}", self.gc_content_pct),
        ];

        if let Some(lp) = &self.length_percentiles {
            headers.extend([
                "len_p10", "len_p25", "len_p50", "len_p75", "len_p90", "len_p99",
            ]);
            values.extend([
                format!("{:.1}", lp.p10),
                format!("{:.1}", lp.p25),
                format!("{:.1}", lp.p50),
                format!("{:.1}", lp.p75),
                format!("{:.1}", lp.p90),
                format!("{:.1}", lp.p99),
            ]);
        }
        if let Some(qp) = &self.quality_percentiles {
            headers.extend([
                "qual_p10", "qual_p25", "qual_p50", "qual_p75", "qual_p90", "qual_p99",
            ]);
            values.extend([
                format!("{:.2}", qp.p10),
                format!("{:.2}", qp.p25),
                format!("{:.2}", qp.p50),
                format!("{:.2}", qp.p75),
                format!("{:.2}", qp.p90),
                format!("{:.2}", qp.p99),
            ]);
        }

        writeln!(w, "{}", headers.join("\t"))?;
        writeln!(w, "{}", values.join("\t"))?;
        Ok(())
    }
}

fn fmt_int(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}
