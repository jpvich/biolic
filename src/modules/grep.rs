//! `biolic grep`: search reads by sequence or name and keep the matches.
//!
//! A transform module like `filter`: each record is tested independently and,
//! if it matches (or, with `-v`, if it does not), written to stdout in the input
//! format. Matching is case-insensitive by default (`--case-sensitive` to flip).
//!
//! Match modes (the pure [`Matcher`] core picks a backend):
//! - exact substring, single (`-p`) or many (`-f`), via Aho-Corasick;
//! - regular expressions (`--regex`), via `regex::bytes::RegexSet`;
//! - approximate (`--mismatches N`, Hamming substitutions) and IUPAC degenerate
//!   (`-d`, e.g. `N`=any, `R`=A/G), via a sliding-window fuzzy engine;
//! - either strand (`--both-strands`) by also testing the reverse complement.
//!
//! `--regex` is mutually exclusive with `--mismatches`/`--degenerate` (regex
//! already provides flexible matching); the others compose. The logic lives in
//! [`Matcher`] (no I/O); `run` only wires reader → core → writer.

use std::io::BufWriter;
use std::path::PathBuf;

use aho_corasick::{AhoCorasick, AhoCorasickBuilder};
use anyhow::{anyhow, bail, Context, Result};
use clap::{ArgGroup, Args};
use regex::bytes::{RegexSet, RegexSetBuilder};

use crate::cli::RunContext;
use crate::io::reader::open_input;
use crate::io::{FastaWriter, FastqWriter, Format, RecordWriter};
use crate::output::fmt_commas;
use crate::record::Record;
use crate::utils::dna::{iupac_matches, reverse_complement};

#[derive(Args, Debug)]
#[command(group(
    ArgGroup::new("patterns")
        .required(true)
        .multiple(false)
        .args(["pattern", "patterns_file"])
))]
pub struct GrepArgs {
    /// Input file (FASTQ, FASTA, or unaligned BAM; optionally gzipped).
    /// Use "-" or omit for stdin.
    #[arg(default_value = "-")]
    pub input: PathBuf,

    /// A single search pattern.
    #[arg(short = 'p', long)]
    pub pattern: Option<String>,

    /// File of patterns, one per line (blank lines and lines starting with '#'
    /// are ignored). Matches any of them.
    #[arg(short = 'f', long)]
    pub patterns_file: Option<PathBuf>,

    /// Interpret the pattern(s) as regular expressions.
    #[arg(long, conflicts_with_all = ["mismatches", "degenerate"])]
    pub regex: bool,

    /// Allow up to N mismatches (substitutions). Approximate matching.
    #[arg(long, value_name = "N")]
    pub mismatches: Option<usize>,

    /// Interpret IUPAC ambiguity codes in the pattern (N=any, R=A/G, Y=C/T, …).
    #[arg(short = 'd', long)]
    pub degenerate: bool,

    /// Search read names instead of sequences.
    #[arg(short = 'n', long, conflicts_with = "both_strands")]
    pub in_name: bool,

    /// Also match the reverse complement (sequence search only).
    #[arg(long)]
    pub both_strands: bool,

    /// Output records that do NOT match.
    #[arg(short = 'v', long)]
    pub invert: bool,

    /// Match case-sensitively (matching is case-insensitive by default).
    #[arg(long)]
    pub case_sensitive: bool,
}

pub fn run(args: GrepArgs, ctx: &RunContext) -> Result<()> {
    let patterns = load_patterns(&args)?;
    let matcher = Matcher::new(&patterns, opts_from_args(&args))?;

    let mut reader = open_input(&args.input)
        .with_context(|| format!("opening input {}", args.input.display()))?;
    let format = reader.format();

    let stdout = std::io::stdout();
    let handle = BufWriter::new(stdout.lock());
    let mut writer: Box<dyn RecordWriter> = match format {
        Format::Fasta => Box::new(FastaWriter::new(handle)),
        Format::Fastq | Format::Bam => Box::new(FastqWriter::new(handle)),
    };

    // The only I/O loop; the match decision is in the pure `Matcher`.
    let mut total = 0u64;
    let mut kept = 0u64;
    while let Some(rec) = reader.next_record().context("reading record")? {
        total += 1;
        if matcher.matches(&rec) != args.invert {
            writer.write_record(&rec)?;
            kept += 1;
        }
    }
    writer.flush()?;

    if !ctx.quiet {
        eprintln!(
            "[biolic grep] kept {} / {} reads",
            fmt_commas(kept),
            fmt_commas(total)
        );
    }
    Ok(())
}

/// Read the patterns from `-p` or the `-f` file (the only pattern-side I/O).
fn load_patterns(args: &GrepArgs) -> Result<Vec<String>> {
    if let Some(p) = &args.pattern {
        if p.is_empty() {
            bail!("--pattern must not be empty");
        }
        Ok(vec![p.clone()])
    } else if let Some(path) = &args.patterns_file {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading patterns file {}", path.display()))?;
        let patterns: Vec<String> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect();
        if patterns.is_empty() {
            bail!("no patterns found in {}", path.display());
        }
        Ok(patterns)
    } else {
        // The clap ArgGroup guarantees exactly one source is present.
        unreachable!("clap requires one of --pattern / --patterns-file")
    }
}

fn opts_from_args(args: &GrepArgs) -> MatchOpts {
    MatchOpts {
        target: if args.in_name {
            Target::Name
        } else {
            Target::Sequence
        },
        regex: args.regex,
        mismatches: args.mismatches.unwrap_or(0),
        degenerate: args.degenerate,
        both_strands: args.both_strands,
        case_insensitive: !args.case_sensitive,
    }
}

/// What part of the record to search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Sequence,
    Name,
}

/// Resolved matching options (clap-free).
#[derive(Debug, Clone)]
pub struct MatchOpts {
    pub target: Target,
    pub regex: bool,
    pub mismatches: usize,
    pub degenerate: bool,
    pub both_strands: bool,
    pub case_insensitive: bool,
}

/// The pure search core: build it from patterns + options, then call
/// [`Matcher::matches`] per record. No I/O; this is the unit-tested,
/// Python-bindable part. `run` is the thin wrapper around it.
pub struct Matcher {
    backend: Backend,
    target: Target,
    both_strands: bool,
}

enum Backend {
    /// Exact substring(s), `mismatches == 0 && !degenerate`. Fast multi-pattern.
    Exact(AhoCorasick),
    /// Regular expressions.
    Regex(RegexSet),
    /// Sliding-window Hamming search with optional IUPAC set membership.
    Fuzzy {
        patterns: Vec<Vec<u8>>,
        mismatches: usize,
        degenerate: bool,
        case_insensitive: bool,
    },
}

impl Matcher {
    pub fn new(patterns: &[String], opts: MatchOpts) -> Result<Self> {
        let backend = if opts.regex {
            let set = RegexSetBuilder::new(patterns)
                .case_insensitive(opts.case_insensitive)
                .build()
                .map_err(|e| anyhow!("invalid regex: {e}"))?;
            Backend::Regex(set)
        } else if opts.mismatches == 0 && !opts.degenerate {
            let ac = AhoCorasickBuilder::new()
                .ascii_case_insensitive(opts.case_insensitive)
                .build(patterns)
                .map_err(|e| anyhow!("failed to build pattern matcher: {e}"))?;
            Backend::Exact(ac)
        } else {
            Backend::Fuzzy {
                patterns: patterns.iter().map(|p| p.as_bytes().to_vec()).collect(),
                mismatches: opts.mismatches,
                degenerate: opts.degenerate,
                case_insensitive: opts.case_insensitive,
            }
        };
        Ok(Self {
            backend,
            target: opts.target,
            both_strands: opts.both_strands,
        })
    }

    /// Whether the record matches (before any `--invert` is applied).
    pub fn matches(&self, rec: &Record) -> bool {
        match self.target {
            Target::Name => self.hits(&rec.id),
            Target::Sequence => {
                if self.hits(&rec.seq) {
                    return true;
                }
                self.both_strands && self.hits(&reverse_complement(&rec.seq))
            }
        }
    }

    /// Whether any pattern is found in a single byte string (one strand).
    fn hits(&self, hay: &[u8]) -> bool {
        match &self.backend {
            Backend::Exact(ac) => ac.is_match(hay),
            Backend::Regex(set) => set.is_match(hay),
            Backend::Fuzzy {
                patterns,
                mismatches,
                degenerate,
                case_insensitive,
            } => patterns
                .iter()
                .any(|pat| fuzzy_contains(hay, pat, *mismatches, *degenerate, *case_insensitive)),
        }
    }
}

/// Whether `pat` occurs in `hay` within `max_mm` substitutions (Hamming),
/// matching each position by IUPAC membership (`degenerate`) or equality.
fn fuzzy_contains(hay: &[u8], pat: &[u8], max_mm: usize, degenerate: bool, ci: bool) -> bool {
    let plen = pat.len();
    if plen == 0 {
        return true;
    }
    if hay.len() < plen {
        return false;
    }
    for start in 0..=(hay.len() - plen) {
        let mut mm = 0;
        let mut ok = true;
        for (i, &pc) in pat.iter().enumerate() {
            if !pos_match(pc, hay[start + i], degenerate, ci) {
                mm += 1;
                if mm > max_mm {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            return true;
        }
    }
    false
}

/// Match a single pattern byte against an observed base.
#[inline]
fn pos_match(pat: u8, base: u8, degenerate: bool, ci: bool) -> bool {
    if degenerate {
        iupac_matches(pat, base)
    } else if ci {
        pat.eq_ignore_ascii_case(&base)
    } else {
        pat == base
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(target: Target) -> MatchOpts {
        MatchOpts {
            target,
            regex: false,
            mismatches: 0,
            degenerate: false,
            both_strands: false,
            case_insensitive: true,
        }
    }

    fn rec(id: &str, seq: &str) -> Record {
        Record::new(
            id.as_bytes().to_vec(),
            seq.as_bytes().to_vec(),
            Some(vec![b'I'; seq.len()]),
        )
    }

    fn matcher(patterns: &[&str], opts: MatchOpts) -> Matcher {
        let pats: Vec<String> = patterns.iter().map(|s| s.to_string()).collect();
        Matcher::new(&pats, opts).unwrap()
    }

    #[test]
    fn exact_is_case_insensitive_by_default() {
        let m = matcher(&["acgt"], opts(Target::Sequence));
        assert!(m.matches(&rec("r", "TTACGTAA")));
        assert!(!m.matches(&rec("r", "TTAAAA")));
    }

    #[test]
    fn case_sensitive_flag_distinguishes() {
        let mut o = opts(Target::Sequence);
        o.case_insensitive = false;
        let m = matcher(&["acgt"], o);
        assert!(!m.matches(&rec("r", "TTACGTAA")));
        assert!(m.matches(&rec("r", "ttacgtaa")));
    }

    #[test]
    fn multi_pattern_any_hit() {
        let m = matcher(&["GGGG", "TTTT"], opts(Target::Sequence));
        assert!(m.matches(&rec("r", "AAATTTTAAA")));
        assert!(!m.matches(&rec("r", "AAACCCAAA")));
    }

    #[test]
    fn regex_backend() {
        let mut o = opts(Target::Sequence);
        o.regex = true;
        let m = matcher(&["A[CG]GT"], o);
        assert!(m.matches(&rec("r", "TTAGGTAA")));
        assert!(!m.matches(&rec("r", "TTATTTAA")));
    }

    #[test]
    fn mismatches_allows_substitutions() {
        let mut o = opts(Target::Sequence);
        o.mismatches = 1;
        let m = matcher(&["AAAA"], o);
        assert!(m.matches(&rec("r", "TTAAGATT"))); // AAGA: 1 mismatch
        assert!(!m.matches(&rec("r", "TTAGGATT"))); // AGGA: 2 mismatches
    }

    #[test]
    fn degenerate_iupac() {
        let mut o = opts(Target::Sequence);
        o.degenerate = true;
        let m = matcher(&["GGNTGG"], o);
        for b in ["GGATGG", "GGCTGG", "GGGTGG", "GGTTGG"] {
            assert!(m.matches(&rec("r", b)), "{b} should match GGNTGG");
        }
        assert!(
            !m.matches(&rec("r", "GGATTG")),
            "second-to-last base differs"
        );
    }

    #[test]
    fn both_strands_finds_reverse_complement() {
        // Forward pattern absent on the given strand, present on its revcomp.
        let mut o = opts(Target::Sequence);
        o.both_strands = true;
        let m = matcher(&["ACGT"], o);
        // revcomp("AAACGTAA") contains... use a seq whose forward lacks ACGT but
        // revcomp contains it. revcomp("TTACGTTT") = "AAACGTAA" -> contains ACGT.
        let seq = "TTACGTTT";
        assert!(m.matches(&rec("r", seq)));
        // Without both_strands and case-sensitive, a seq lacking ACGT on either
        // tested strand should not match.
        let m1 = matcher(&["GGGG"], opts(Target::Sequence));
        assert!(!m1.matches(&rec("r", "AAAACCCC")));
    }

    #[test]
    fn name_target_searches_id_not_sequence() {
        let m = matcher(&["read1"], opts(Target::Name));
        assert!(m.matches(&rec("read1", "AAAA")));
        assert!(!m.matches(&rec("read2", "read1")));
    }
}
