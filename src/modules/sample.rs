//! `biolic sample`: subsample reads by count, proportion, total bases, or
//! target coverage.
//!
//! Modes (mutually exclusive):
//! - `-n N`        exactly N reads, via reservoir sampling (Algorithm R).
//! - `-p FRACTION` keep each read with probability FRACTION (Bernoulli).
//! - `--bases SIZE` / `--coverage X --genome-size SIZE` — sample down to a
//!   target number of bases. Implemented as a quick first pass to total the
//!   bases, then Bernoulli sampling at `target / total`. This is rasusa's
//!   differentiator (neither nanoq nor seqkit do coverage-target sampling).
//!
//! `-n` and `-p` stream in a single pass (reservoir holds the output; Bernoulli
//! is O(1)). The base/coverage modes need two passes, so they require a file
//! input, not stdin. `--seed` makes any mode reproducible.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Args;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::cli::RunContext;
use crate::io::reader::open_input;
use crate::io::{FastaWriter, FastqWriter, Format, RecordReader, RecordWriter};
use crate::output::fmt_commas;
use crate::record::Record;

#[derive(Args, Debug)]
pub struct SampleArgs {
    /// Input file (FASTQ/FASTA/BAM, optionally gzipped). "-" or omit for stdin.
    #[arg(default_value = "-")]
    pub input: PathBuf,

    /// Keep exactly this many reads (reservoir sampling).
    #[arg(short = 'n', long, conflicts_with_all = &["proportion", "bases", "coverage"])]
    pub count: Option<u64>,

    /// Keep each read with this probability (0.0-1.0).
    #[arg(short = 'p', long, conflicts_with_all = &["count", "bases", "coverage"])]
    pub proportion: Option<f64>,

    /// Sample down to roughly this many bases (e.g. 1G, 500M). Requires a file.
    #[arg(long, conflicts_with_all = &["count", "proportion", "coverage"])]
    pub bases: Option<String>,

    /// Sample down to this target coverage (needs --genome-size). Requires a file.
    #[arg(long, requires = "genome_size")]
    pub coverage: Option<f64>,

    /// Genome size for --coverage (e.g. 5M, 3.2G).
    #[arg(long)]
    pub genome_size: Option<String>,

    /// Random seed for reproducible sampling.
    #[arg(long)]
    pub seed: Option<u64>,
}

/// The resolved sampling mode after validating the flags.
#[derive(Debug, PartialEq)]
enum Mode {
    Count(u64),
    Proportion(f64),
    /// A target number of bases (from --bases or --coverage × --genome-size).
    TargetBases(u64),
}

pub fn run(args: SampleArgs, ctx: &RunContext) -> Result<()> {
    let mode = resolve_mode(&args)?;
    let mut rng = make_rng(args.seed);

    // Base/coverage modes need the total base count first, which means a second
    // pass — only possible on a real file.
    let fraction = if let Mode::TargetBases(target) = mode {
        if args.input == Path::new("-") {
            bail!("--bases/--coverage require a file input (cannot make two passes over stdin)");
        }
        let total = total_bases(&args.input)?;
        if total == 0 {
            0.0
        } else {
            (target as f64 / total as f64).min(1.0)
        }
    } else {
        0.0
    };

    let mut reader = open_input(&args.input)
        .with_context(|| format!("opening input {}", args.input.display()))?;
    let format = reader.format();

    let stdout = std::io::stdout();
    let handle = std::io::BufWriter::new(stdout.lock());
    let mut writer: Box<dyn RecordWriter> = match format {
        Format::Fasta => Box::new(FastaWriter::new(handle)),
        Format::Fastq | Format::Bam => Box::new(FastqWriter::new(handle)),
    };

    let mut total = 0u64;
    let kept;

    match mode {
        Mode::Count(n) => {
            // The pure `Reservoir` core decides selection (Algorithm R); the
            // loop here is the only I/O.
            let mut reservoir = Reservoir::new(n as usize);
            while let Some(rec) = reader.next_record().context("reading record")? {
                reservoir.offer(rec, &mut rng);
            }
            total = reservoir.total_seen();
            let records = reservoir.into_records();
            kept = records.len() as u64;
            for rec in &records {
                writer.write_record(rec)?;
            }
        }
        Mode::Proportion(p) => {
            kept = bernoulli_stream(reader.as_mut(), writer.as_mut(), p, &mut rng, &mut total)?;
        }
        Mode::TargetBases(_) => {
            kept = bernoulli_stream(
                reader.as_mut(),
                writer.as_mut(),
                fraction,
                &mut rng,
                &mut total,
            )?;
        }
    }

    writer.flush()?;
    if !ctx.quiet {
        eprintln!(
            "[biolic sample] kept {} / {} reads",
            fmt_commas(kept),
            fmt_commas(total)
        );
    }
    Ok(())
}

/// Reservoir sampler (Algorithm R): a pure, I/O-free selection over a record
/// stream. Offer records one by one; it keeps a uniform random subset of at
/// most `capacity`, in O(capacity) memory and a single pass. Future Python
/// bindings can drive this directly over an iterator of records.
pub struct Reservoir {
    capacity: usize,
    seen: u64,
    items: Vec<Record>,
}

impl Reservoir {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            seen: 0,
            items: Vec::with_capacity(capacity.min(1024)),
        }
    }

    /// Offer one record from the stream. `seen` is the number of records offered
    /// so far (Algorithm R's index `i`), used as the replacement bound.
    pub fn offer(&mut self, rec: Record, rng: &mut StdRng) {
        if self.items.len() < self.capacity {
            self.items.push(rec);
        } else if self.capacity > 0 {
            let j = rng.gen_range(0..=self.seen);
            if (j as usize) < self.capacity {
                self.items[j as usize] = rec;
            }
        }
        self.seen += 1;
    }

    /// Total records offered so far.
    pub fn total_seen(&self) -> u64 {
        self.seen
    }

    /// Consume the reservoir, returning the selected records.
    pub fn into_records(self) -> Vec<Record> {
        self.items
    }
}

/// Bernoulli pass: emit each read with probability `p`. Returns the kept count
/// and writes the total seen into `total`. The selection itself is a single
/// `rng.gen_bool(p)`; this thin loop is the I/O wiring used by `run`.
fn bernoulli_stream(
    reader: &mut dyn RecordReader,
    writer: &mut dyn RecordWriter,
    p: f64,
    rng: &mut StdRng,
    total: &mut u64,
) -> Result<u64> {
    let mut kept = 0u64;
    while let Some(rec) = reader.next_record().context("reading record")? {
        *total += 1;
        if rng.gen_bool(p) {
            writer.write_record(&rec)?;
            kept += 1;
        }
    }
    Ok(kept)
}

/// First pass for base/coverage modes: total the bases, O(1) memory.
fn total_bases(input: &Path) -> Result<u64> {
    let mut reader =
        open_input(input).with_context(|| format!("opening input {}", input.display()))?;
    let mut total = 0u64;
    while let Some(rec) = reader.next_record().context("reading record")? {
        total += rec.len() as u64;
    }
    Ok(total)
}

/// Build a seeded RNG, or one seeded from system entropy when no seed is given.
fn make_rng(seed: Option<u64>) -> StdRng {
    match seed {
        Some(s) => StdRng::seed_from_u64(s),
        None => StdRng::from_entropy(),
    }
}

/// Resolve and validate the sampling mode from the parsed flags.
fn resolve_mode(args: &SampleArgs) -> Result<Mode> {
    if let Some(n) = args.count {
        return Ok(Mode::Count(n));
    }
    if let Some(p) = args.proportion {
        if !(0.0..=1.0).contains(&p) {
            bail!("--proportion must be between 0.0 and 1.0 (got {p})");
        }
        return Ok(Mode::Proportion(p));
    }
    if let Some(b) = &args.bases {
        return Ok(Mode::TargetBases(parse_size(b)?));
    }
    if let Some(cov) = args.coverage {
        if cov <= 0.0 {
            bail!("--coverage must be positive (got {cov})");
        }
        let gs = args
            .genome_size
            .as_ref()
            .expect("clap enforces --genome-size with --coverage");
        let genome = parse_size(gs)?;
        return Ok(Mode::TargetBases((cov * genome as f64).round() as u64));
    }
    bail!("specify one of: -n/--count, -p/--proportion, --bases, or --coverage --genome-size")
}

/// Parse a human size with an optional K/M/G suffix (base 1000), e.g. `1.5G`.
fn parse_size(s: &str) -> Result<u64> {
    let s = s.trim();
    if s.is_empty() {
        bail!("empty size value");
    }
    let last = s.chars().last().unwrap();
    let (num, mult): (&str, u64) = match last.to_ascii_uppercase() {
        'K' => (&s[..s.len() - 1], 1_000),
        'M' => (&s[..s.len() - 1], 1_000_000),
        'G' => (&s[..s.len() - 1], 1_000_000_000),
        c if c.is_ascii_digit() => (s, 1),
        other => bail!("invalid size suffix '{other}' in '{s}': use K, M, or G"),
    };
    let value: f64 = num
        .trim()
        .parse()
        .with_context(|| format!("invalid size '{s}'"))?;
    if value < 0.0 {
        bail!("size must not be negative: '{s}'");
    }
    Ok((value * mult as f64).round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_args() -> SampleArgs {
        SampleArgs {
            input: PathBuf::from("-"),
            count: None,
            proportion: None,
            bases: None,
            coverage: None,
            genome_size: None,
            seed: None,
        }
    }

    #[test]
    fn parse_size_handles_suffixes() {
        assert_eq!(parse_size("100").unwrap(), 100);
        assert_eq!(parse_size("1K").unwrap(), 1_000);
        assert_eq!(parse_size("500M").unwrap(), 500_000_000);
        assert_eq!(parse_size("1G").unwrap(), 1_000_000_000);
        assert_eq!(parse_size("1.5g").unwrap(), 1_500_000_000);
        assert!(parse_size("12Q").is_err());
        assert!(parse_size("").is_err());
    }

    #[test]
    fn resolve_mode_picks_count() {
        let a = SampleArgs {
            count: Some(10),
            ..base_args()
        };
        assert_eq!(resolve_mode(&a).unwrap(), Mode::Count(10));
    }

    #[test]
    fn resolve_mode_rejects_bad_proportion() {
        let a = SampleArgs {
            proportion: Some(1.5),
            ..base_args()
        };
        assert!(resolve_mode(&a).is_err());
    }

    #[test]
    fn resolve_mode_coverage_multiplies_genome_size() {
        let a = SampleArgs {
            coverage: Some(30.0),
            genome_size: Some("5M".to_string()),
            ..base_args()
        };
        // 30 × 5,000,000 = 150,000,000 target bases.
        assert_eq!(resolve_mode(&a).unwrap(), Mode::TargetBases(150_000_000));
    }

    #[test]
    fn resolve_mode_requires_a_mode() {
        assert!(resolve_mode(&base_args()).is_err());
    }

    fn numbered(i: u8) -> Record {
        Record::new(
            vec![b'r', b'0' + i],
            b"ACGT".to_vec(),
            Some(b"IIII".to_vec()),
        )
    }

    fn reservoir_run(capacity: usize, count: u8, seed: u64) -> Vec<Vec<u8>> {
        let mut rng = StdRng::seed_from_u64(seed);
        let mut r = Reservoir::new(capacity);
        for i in 0..count {
            r.offer(numbered(i), &mut rng);
        }
        assert_eq!(r.total_seen(), count as u64);
        r.into_records().into_iter().map(|rec| rec.id).collect()
    }

    #[test]
    fn reservoir_keeps_capacity_and_is_deterministic() {
        let a = reservoir_run(2, 5, 7);
        assert_eq!(a.len(), 2);
        // Same seed → identical selection.
        assert_eq!(a, reservoir_run(2, 5, 7));
    }

    #[test]
    fn reservoir_capacity_zero_and_full() {
        assert_eq!(reservoir_run(0, 5, 1).len(), 0);
        // Capacity ≥ stream size keeps everything.
        assert_eq!(reservoir_run(10, 3, 1).len(), 3);
    }
}
