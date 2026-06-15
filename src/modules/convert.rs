//! `biolic convert`: convert between sequence formats and compression.
//!
//! Supported in v1: BAM → FASTQ/FASTA, FASTQ ↔ FASTA, and gzip (de)compression.
//! The target format and gzip are inferred from the `-o` extension; writing to
//! stdout (`-o -`, the default) needs `--to`. Reuses the shared `io::writer`.
//!
//! Streaming throughout — never buffers the whole file. Stripping quality
//! (→ FASTA) is lossless-by-design; the reverse (FASTA → FASTQ) fabricates
//! quality and is refused unless `--fake-quality` is given explicitly.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, ValueEnum};
use flate2::write::GzEncoder;
use flate2::Compression;

use crate::cli::RunContext;
use crate::io::reader::open_input;
use crate::io::{FastaWriter, FastqWriter, RecordWriter};
use crate::output::fmt_commas;
use crate::record::Record;

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum TargetFormat {
    Fastq,
    Fasta,
}

#[derive(Args, Debug)]
pub struct ConvertArgs {
    /// Input file (FASTQ/FASTA/BAM, optionally gzipped). "-" or omit for stdin.
    #[arg(default_value = "-")]
    pub input: PathBuf,

    /// Output file; format and gzip are inferred from its extension.
    /// "-" (the default) writes to stdout — then set the format with --to.
    #[arg(short = 'o', long, default_value = "-")]
    pub output: PathBuf,

    /// Target format, required when it can't be inferred from -o (e.g. stdout).
    #[arg(long, value_enum)]
    pub to: Option<TargetFormat>,

    /// Quality (Phred) to assign when converting a quality-less input (FASTA)
    /// to FASTQ. Without it, FASTA → FASTQ is refused rather than fabricating data.
    #[arg(long, value_name = "PHRED")]
    pub fake_quality: Option<u8>,
}

pub fn run(args: ConvertArgs, ctx: &RunContext) -> Result<()> {
    reject_unsupported_compression(&args.output)?;
    let target = resolve_target(&args)?;
    let gzip = is_gzip_path(&args.output);

    let mut reader = open_input(&args.input)
        .with_context(|| format!("opening input {}", args.input.display()))?;

    // Fail fast: a quality-less input cannot become FASTQ without fabricated
    // quality. (BAM normally carries quality; the per-record guard covers the
    // rare BAM-without-quality case.)
    if target == TargetFormat::Fastq
        && reader.format() == crate::io::Format::Fasta
        && args.fake_quality.is_none()
    {
        bail!(
            "converting a quality-less input to FASTQ requires --fake-quality PHRED \
             (e.g. --fake-quality 30)"
        );
    }

    let sink = open_output(&args.output, gzip)
        .with_context(|| format!("creating output {}", args.output.display()))?;
    let mut writer: Box<dyn RecordWriter> = match target {
        TargetFormat::Fastq => Box::new(FastqWriter::new(sink)),
        TargetFormat::Fasta => Box::new(FastaWriter::new(sink)),
    };

    // The per-record transform is the pure core; this loop is the only I/O.
    let converter = Converter {
        target,
        fake_quality: args.fake_quality,
    };
    let mut written = 0u64;
    while let Some(rec) = reader.next_record().context("reading record")? {
        let out = converter.apply(rec)?;
        writer.write_record(&out)?;
        written += 1;
    }
    writer.flush()?;

    if !ctx.quiet {
        eprintln!("[biolic convert] wrote {} records", fmt_commas(written));
    }
    Ok(())
}

/// The pure conversion core: a per-record transform with no I/O. Build it from
/// the resolved target and options, then call [`apply`](Converter::apply) on
/// each record. `run` only handles reading, output setup, and writing. Future
/// Python bindings wrap this same type.
#[derive(Debug, Clone)]
pub struct Converter {
    pub target: TargetFormat,
    pub fake_quality: Option<u8>,
}

impl Converter {
    /// Transform one record for the target format. For a FASTQ target, a
    /// quality-less record gets fabricated quality from `fake_quality`, or this
    /// errors if that flag was not supplied.
    pub fn apply(&self, mut rec: Record) -> Result<Record> {
        if self.target == TargetFormat::Fastq && rec.qual.is_none() {
            let q = self.fake_quality.ok_or_else(|| {
                anyhow!(
                    "record '{}' has no quality; converting to FASTQ requires --fake-quality PHRED",
                    String::from_utf8_lossy(&rec.id)
                )
            })?;
            rec.qual = Some(vec![q.saturating_add(33); rec.seq.len()]);
        }
        Ok(rec)
    }
}

/// Resolve the target format from `--to`, else infer it from the `-o` extension.
fn resolve_target(args: &ConvertArgs) -> Result<TargetFormat> {
    if let Some(t) = args.to {
        return Ok(t);
    }
    infer_format_from_path(&args.output).ok_or_else(|| {
        anyhow!(
            "cannot infer output format from {}; pass --to <fastq|fasta>",
            args.output.display()
        )
    })
}

/// Infer FASTQ/FASTA from a filename, ignoring a trailing `.gz`.
fn infer_format_from_path(p: &Path) -> Option<TargetFormat> {
    let name = p.file_name()?.to_str()?.to_lowercase();
    let stem = name.strip_suffix(".gz").unwrap_or(&name);
    if stem.ends_with(".fastq") || stem.ends_with(".fq") {
        Some(TargetFormat::Fastq)
    } else if stem.ends_with(".fasta") || stem.ends_with(".fa") || stem.ends_with(".fna") {
        Some(TargetFormat::Fasta)
    } else {
        None
    }
}

/// Whether the output path requests gzip compression (`.gz`).
fn is_gzip_path(p: &Path) -> bool {
    p.file_name()
        .and_then(|s| s.to_str())
        .map(|n| n.to_lowercase().ends_with(".gz"))
        .unwrap_or(false)
}

/// Error early on compressed-output formats we don't write yet (bz2/xz/zst).
fn reject_unsupported_compression(p: &Path) -> Result<()> {
    if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
        let lower = name.to_lowercase();
        for ext in [".bz2", ".xz", ".zst"] {
            if lower.ends_with(ext) {
                bail!(
                    "compressed output '{}' is not supported yet; write plain or gzip (.gz)",
                    ext
                );
            }
        }
    }
    Ok(())
}

/// Open the output sink: stdout for "-", otherwise a file, gzip-wrapped if asked.
fn open_output(path: &Path, gzip: bool) -> Result<Box<dyn Write>> {
    if path == Path::new("-") {
        Ok(Box::new(BufWriter::new(std::io::stdout())))
    } else {
        let file = File::create(path)?;
        if gzip {
            Ok(Box::new(BufWriter::new(GzEncoder::new(
                file,
                Compression::default(),
            ))))
        } else {
            Ok(Box::new(BufWriter::new(file)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_format_from_extension() {
        assert_eq!(
            infer_format_from_path(Path::new("out.fastq")),
            Some(TargetFormat::Fastq)
        );
        assert_eq!(
            infer_format_from_path(Path::new("out.fq.gz")),
            Some(TargetFormat::Fastq)
        );
        assert_eq!(
            infer_format_from_path(Path::new("out.fasta")),
            Some(TargetFormat::Fasta)
        );
        assert_eq!(
            infer_format_from_path(Path::new("out.fa.gz")),
            Some(TargetFormat::Fasta)
        );
        assert_eq!(infer_format_from_path(Path::new("out.txt")), None);
    }

    #[test]
    fn detects_gzip_extension() {
        assert!(is_gzip_path(Path::new("out.fastq.gz")));
        assert!(!is_gzip_path(Path::new("out.fastq")));
        assert!(!is_gzip_path(Path::new("-")));
    }

    #[test]
    fn rejects_other_compression() {
        assert!(reject_unsupported_compression(Path::new("out.fastq.bz2")).is_err());
        assert!(reject_unsupported_compression(Path::new("out.fastq.xz")).is_err());
        assert!(reject_unsupported_compression(Path::new("out.fastq.gz")).is_ok());
    }

    #[test]
    fn fabricates_quality_only_with_flag() {
        let r = || Record::new(b"r".to_vec(), b"ACGT".to_vec(), None);
        // Without the flag: error.
        let no_flag = Converter {
            target: TargetFormat::Fastq,
            fake_quality: None,
        };
        assert!(no_flag.apply(r()).is_err());
        // With the flag: quality filled to the sequence length.
        let with_flag = Converter {
            target: TargetFormat::Fastq,
            fake_quality: Some(30),
        };
        let out = with_flag.apply(r()).unwrap();
        assert_eq!(out.qual.as_ref().unwrap().len(), 4);
        assert_eq!(out.qual.unwrap()[0], 30 + 33);
    }

    #[test]
    fn to_fasta_leaves_quality_untouched() {
        // FASTA target never fabricates quality.
        let c = Converter {
            target: TargetFormat::Fasta,
            fake_quality: None,
        };
        let out = c
            .apply(Record::new(b"r".to_vec(), b"ACGT".to_vec(), None))
            .unwrap();
        assert!(out.qual.is_none());
    }
}
