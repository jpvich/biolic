//! Unified file reader.
//!
//! Provides format detection, transparent gzip decompression, and a uniform
//! `RecordReader` trait that all modules consume. The sole sequence parser is
//! `noodles` (decision D2). Supports FASTQ and FASTA (both plain and gzip).
//! Unaligned BAM is added next in Milestone 1.
//!
//! Gzip is handled with the pure-Rust `flate2` crate so the default binary is
//! fully static (decision D3). bz2/xz/zstd are only available when built with
//! the `extra-compression` feature.

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use anyhow::{anyhow, Context, Result};
use flate2::read::MultiGzDecoder;
// Brings the `iter`/`len`/`is_empty` methods on BAM quality scores into scope.
use noodles::sam::alignment::record::QualityScores as _;

use crate::record::Record;

/// A streaming reader of `Record`s.
///
/// All format-specific readers implement this trait. Modules consume records
/// without needing to know the underlying format.
pub trait RecordReader {
    /// Return the next record, or `None` at EOF.
    fn next_record(&mut self) -> Result<Option<Record>>;

    /// The sequence format this reader parses.
    fn format(&self) -> Format;
}

/// Detected file format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Fastq,
    Fasta,
    Bam,
}

impl Format {
    /// Short uppercase name for display (e.g. in the `stats` table).
    pub fn name(&self) -> &'static str {
        match self {
            Format::Fastq => "FASTQ",
            Format::Fasta => "FASTA",
            Format::Bam => "BAM",
        }
    }
}

/// Open a file (or stdin if path is "-") and return a boxed `RecordReader`.
///
/// Format detection is by file extension. Compression (gzip) is auto-detected
/// from the stream's magic bytes, so it works regardless of extension.
pub fn open_reader<P: AsRef<Path>>(path: P) -> Result<Box<dyn RecordReader>> {
    let path = path.as_ref();
    let format = detect_format(path)?;

    match format {
        Format::Fastq => {
            let reader = FastqReader::open(path)?;
            Ok(Box::new(reader))
        }
        Format::Fasta => {
            let reader = FastaReader::open(path)?;
            Ok(Box::new(reader))
        }
        Format::Bam => {
            let reader = BamReader::open(path)?;
            Ok(Box::new(reader))
        }
    }
}

/// Detect format from file extension.
///
/// Recognized extensions:
/// - `.fastq`, `.fq` (+ `.gz`/`.bz2`/`.xz`/`.zst`) -> FASTQ
/// - `.fasta`, `.fa`, `.fna` (+ `.gz`) -> FASTA
/// - `.bam`, `.sam`, `.cram` -> BAM
pub fn detect_format(path: &Path) -> Result<Format> {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| anyhow!("Invalid file path: {}", path.display()))?
        .to_lowercase();

    // Strip a trailing compression extension so the underlying format extension
    // is what we match against.
    let stem = strip_compression_ext(&name);

    if stem.ends_with(".fastq") || stem.ends_with(".fq") {
        Ok(Format::Fastq)
    } else if stem.ends_with(".fasta") || stem.ends_with(".fa") || stem.ends_with(".fna") {
        Ok(Format::Fasta)
    } else if stem.ends_with(".bam") || stem.ends_with(".sam") || stem.ends_with(".cram") {
        Ok(Format::Bam)
    } else {
        Err(anyhow!(
            "Could not detect format from filename: {}. \
             Supported extensions: .fastq[.gz], .fq[.gz], .fasta[.gz], .fa[.gz], .bam, .sam, .cram",
            path.display()
        ))
    }
}

/// Remove a single trailing compression suffix from a (lowercased) filename.
fn strip_compression_ext(name: &str) -> &str {
    for suffix in [".gz", ".bz2", ".xz", ".zst"] {
        if let Some(stripped) = name.strip_suffix(suffix) {
            return stripped;
        }
    }
    name
}

/// Wrap a reader, transparently decompressing if the stream begins with the
/// gzip magic bytes (0x1f 0x8b). Returns a `BufRead` suitable for noodles.
fn decompressed(reader: impl Read + 'static) -> Result<Box<dyn BufRead>> {
    let mut buf = BufReader::new(reader);
    // Peek without consuming so the decoder still sees the magic bytes.
    let is_gzip = {
        let head = buf.fill_buf().context("reading input header")?;
        head.len() >= 2 && head[0] == 0x1f && head[1] == 0x8b
    };
    if is_gzip {
        Ok(Box::new(BufReader::new(MultiGzDecoder::new(buf))))
    } else {
        Ok(Box::new(buf))
    }
}

/// FASTQ reader built on `noodles::fastq` with pure-Rust gzip support.
pub struct FastqReader {
    inner: noodles::fastq::io::Reader<Box<dyn BufRead>>,
}

impl FastqReader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let file =
            File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;
        Self::from_reader(file)
    }

    /// Open from any reader (used for stdin).
    pub fn from_reader<R: Read + 'static>(reader: R) -> Result<Self> {
        let inner = decompressed(reader)?;
        Ok(Self {
            inner: noodles::fastq::io::Reader::new(inner),
        })
    }
}

impl RecordReader for FastqReader {
    fn next_record(&mut self) -> Result<Option<Record>> {
        let mut rec = noodles::fastq::Record::default();
        let n = self
            .inner
            .read_record(&mut rec)
            .map_err(|e| anyhow!("FASTQ parse error: {}", e))?;
        if n == 0 {
            return Ok(None);
        }
        let id = rec.name().to_vec();
        let seq = rec.sequence().to_vec();
        let qual_bytes = rec.quality_scores().to_vec();
        // A valid FASTQ record has equal sequence and quality lengths. noodles
        // does not enforce this, so guard against malformed input here.
        if qual_bytes.len() != seq.len() {
            return Err(anyhow!(
                "malformed FASTQ record '{}': sequence length {} != quality length {}",
                String::from_utf8_lossy(&id),
                seq.len(),
                qual_bytes.len()
            ));
        }
        let qual = Some(qual_bytes);
        let desc = {
            let d = rec.description();
            if d.is_empty() {
                None
            } else {
                Some(d.to_vec())
            }
        };
        Ok(Some(Record::with_desc(id, seq, qual, desc)))
    }

    fn format(&self) -> Format {
        Format::Fastq
    }
}

/// FASTA reader built on `noodles::fasta` with pure-Rust gzip support.
///
/// FASTA carries no quality scores, so `Record::qual` is always `None`.
pub struct FastaReader {
    inner: noodles::fasta::io::Reader<Box<dyn BufRead>>,
}

impl FastaReader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let file =
            File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;
        Self::from_reader(file)
    }

    /// Open from any reader (used for stdin).
    pub fn from_reader<R: Read + 'static>(reader: R) -> Result<Self> {
        let inner = decompressed(reader)?;
        Ok(Self {
            inner: noodles::fasta::io::Reader::new(inner),
        })
    }
}

impl RecordReader for FastaReader {
    fn next_record(&mut self) -> Result<Option<Record>> {
        let mut definition = String::new();
        let n = self
            .inner
            .read_definition(&mut definition)
            .map_err(|e| anyhow!("FASTA parse error: {}", e))?;
        if n == 0 {
            return Ok(None);
        }

        let mut seq = Vec::new();
        self.inner
            .read_sequence(&mut seq)
            .map_err(|e| anyhow!("FASTA parse error: {}", e))?;

        // The definition line is ">name [optional description]". Tolerate a
        // present-or-absent leading '>' and an optional description.
        let body = definition.trim_end();
        let body = body.strip_prefix('>').unwrap_or(body);
        let (id, desc) = match body.split_once(char::is_whitespace) {
            Some((name, rest)) => (name.as_bytes().to_vec(), Some(rest.as_bytes().to_vec())),
            None => (body.as_bytes().to_vec(), None),
        };

        Ok(Some(Record::with_desc(id, seq, None, desc)))
    }

    fn format(&self) -> Format {
        Format::Fasta
    }
}

/// Unaligned BAM reader built on `noodles::bam`.
///
/// BAM is BGZF-compressed and the bam reader handles that internally, so the
/// gzip auto-detection used for FASTQ/FASTA does not apply here. The SAM header
/// is consumed on open. BAM stores quality as numeric Phred; biolic uses Phred+33
/// ASCII everywhere, so scores are shifted by 33 on read.
pub struct BamReader {
    inner: noodles::bam::io::Reader<noodles::bgzf::Reader<Box<dyn Read>>>,
}

impl BamReader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let file =
            File::open(path).with_context(|| format!("Failed to open file: {}", path.display()))?;
        Self::from_reader(file)
    }

    /// Open from any reader (used for stdin / in-memory data).
    pub fn from_reader<R: Read + 'static>(reader: R) -> Result<Self> {
        let boxed: Box<dyn Read> = Box::new(reader);
        let mut inner = noodles::bam::io::Reader::new(boxed);
        inner
            .read_header()
            .map_err(|e| anyhow!("BAM header error: {}", e))?;
        Ok(Self { inner })
    }
}

impl RecordReader for BamReader {
    fn next_record(&mut self) -> Result<Option<Record>> {
        let mut rec = noodles::bam::Record::default();
        let n = self
            .inner
            .read_record(&mut rec)
            .map_err(|e| anyhow!("BAM parse error: {}", e))?;
        if n == 0 {
            return Ok(None);
        }

        let id = rec.name().map(|name| name.to_vec()).unwrap_or_default();
        let seq: Vec<u8> = rec.sequence().iter().collect();

        let scores = rec.quality_scores();
        let qual = if scores.is_empty() {
            None
        } else {
            let mut v = Vec::with_capacity(scores.len());
            for q in scores.iter() {
                let q = q.map_err(|e| anyhow!("BAM quality error: {}", e))?;
                // BAM stores numeric Phred; biolic uses Phred+33 ASCII.
                v.push(q.saturating_add(33));
            }
            Some(v)
        };

        Ok(Some(Record::new(id, seq, qual)))
    }

    fn format(&self) -> Format {
        Format::Bam
    }
}

/// Detect format from the leading bytes of a stream, for stdin where there is
/// no filename. Recognizes the gzip/BGZF family (0x1f 0x8b), then classifies the
/// (decompressed) content as BAM ("BAM\1"), FASTQ ('@'), or FASTA ('>').
fn sniff_format(head: &[u8]) -> Result<Format> {
    if head.len() >= 2 && head[0] == 0x1f && head[1] == 0x8b {
        // gzip family (plain gzip or BGZF). Decompress a small prefix from the
        // peeked window to classify; a truncated window is fine since the first
        // few decompressed bytes are all we need.
        let mut probe = Vec::with_capacity(16);
        let mut decoder = MultiGzDecoder::new(head);
        let mut byte = [0u8; 1];
        while probe.len() < 16 {
            match decoder.read(&mut byte) {
                Ok(0) => break,
                Ok(_) => probe.push(byte[0]),
                Err(_) => break,
            }
        }
        classify_uncompressed(&probe)
    } else {
        classify_uncompressed(head)
    }
}

/// Classify already-decompressed leading bytes.
fn classify_uncompressed(bytes: &[u8]) -> Result<Format> {
    if bytes.starts_with(b"BAM\x01") {
        return Ok(Format::Bam);
    }
    match bytes.iter().copied().find(|b| !b.is_ascii_whitespace()) {
        Some(b'@') => Ok(Format::Fastq),
        Some(b'>') => Ok(Format::Fasta),
        Some(other) => Err(anyhow!(
            "could not detect format from stdin (first byte {:?}); \
             expected FASTQ ('@'), FASTA ('>'), or BAM",
            other as char
        )),
        // Empty or whitespace-only input: treat as an empty FASTQ stream.
        None => Ok(Format::Fastq),
    }
}

/// Open stdin, auto-detecting the format from its leading bytes.
fn open_stdin() -> Result<Box<dyn RecordReader>> {
    let mut buf = BufReader::new(std::io::stdin());
    let format = {
        let head = buf.fill_buf().context("reading stdin")?;
        if head.is_empty() {
            Format::Fastq
        } else {
            sniff_format(head)?
        }
    };
    // `fill_buf` peeks without consuming, so each reader still sees byte 0.
    match format {
        Format::Fastq => Ok(Box::new(FastqReader::from_reader(buf)?)),
        Format::Fasta => Ok(Box::new(FastaReader::from_reader(buf)?)),
        Format::Bam => Ok(Box::new(BamReader::from_reader(buf)?)),
    }
}

/// Helper: resolve "-" to stdin (format auto-detected), otherwise open the file.
pub fn open_input<P: AsRef<Path>>(path: P) -> Result<Box<dyn RecordReader>> {
    let path = path.as_ref();
    if path == Path::new("-") {
        open_stdin()
    } else {
        open_reader(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_format_fastq() {
        assert_eq!(
            detect_format(Path::new("foo.fastq")).unwrap(),
            Format::Fastq
        );
        assert_eq!(detect_format(Path::new("foo.fq")).unwrap(), Format::Fastq);
        assert_eq!(
            detect_format(Path::new("foo.fastq.gz")).unwrap(),
            Format::Fastq
        );
        assert_eq!(
            detect_format(Path::new("foo.fq.gz")).unwrap(),
            Format::Fastq
        );
        assert_eq!(
            detect_format(Path::new("foo.fastq.bz2")).unwrap(),
            Format::Fastq
        );
    }

    #[test]
    fn test_detect_format_fasta() {
        assert_eq!(
            detect_format(Path::new("foo.fasta")).unwrap(),
            Format::Fasta
        );
        assert_eq!(detect_format(Path::new("foo.fa")).unwrap(), Format::Fasta);
        assert_eq!(detect_format(Path::new("foo.fna")).unwrap(), Format::Fasta);
        assert_eq!(
            detect_format(Path::new("foo.fa.gz")).unwrap(),
            Format::Fasta
        );
    }

    #[test]
    fn test_detect_format_bam() {
        assert_eq!(detect_format(Path::new("foo.bam")).unwrap(), Format::Bam);
        assert_eq!(detect_format(Path::new("foo.sam")).unwrap(), Format::Bam);
    }

    #[test]
    fn test_detect_format_unknown() {
        assert!(detect_format(Path::new("foo.txt")).is_err());
        assert!(detect_format(Path::new("foo")).is_err());
    }

    #[test]
    fn test_bam_read_roundtrip() {
        use noodles::sam::alignment::io::Write as _;
        use noodles::sam::alignment::record::Flags;
        use noodles::sam::alignment::RecordBuf;

        // Build an unaligned BAM in memory (no samtools needed).
        let header = noodles::sam::Header::default();
        let mut buf = Vec::new();
        {
            let mut writer = noodles::bam::io::Writer::new(&mut buf);
            writer.write_header(&header).unwrap();

            let rec1 = RecordBuf::builder()
                .set_flags(Flags::UNMAPPED)
                .set_name("read1")
                .set_sequence(b"ACGT".to_vec().into())
                .set_quality_scores(vec![40u8, 40, 40, 40].into())
                .build();
            writer.write_alignment_record(&header, &rec1).unwrap();

            let rec2 = RecordBuf::builder()
                .set_flags(Flags::UNMAPPED)
                .set_name("read2")
                .set_sequence(b"GGCCAA".to_vec().into())
                .set_quality_scores(vec![20u8, 20, 20, 20, 20, 20].into())
                .build();
            writer.write_alignment_record(&header, &rec2).unwrap();

            writer.try_finish().unwrap();
        }

        // Read it back through BamReader and verify decoding + Phred+33 shift.
        let mut reader = BamReader::from_reader(std::io::Cursor::new(buf)).unwrap();

        let r1 = reader.next_record().unwrap().unwrap();
        assert_eq!(r1.id, b"read1");
        assert_eq!(r1.seq, b"ACGT");
        assert_eq!(r1.qual.unwrap(), vec![40 + 33; 4]);

        let r2 = reader.next_record().unwrap().unwrap();
        assert_eq!(r2.id, b"read2");
        assert_eq!(r2.seq, b"GGCCAA");
        assert_eq!(r2.len(), 6);

        assert!(reader.next_record().unwrap().is_none());
    }

    #[test]
    fn test_fastq_seq_qual_mismatch_errors() {
        // 4 bases but 5 quality chars -> malformed; must error, not miscount.
        let data = b"@r1\nACGT\n+\nIIIII\n";
        let mut reader = FastqReader::from_reader(std::io::Cursor::new(&data[..])).unwrap();
        assert!(reader.next_record().is_err());
    }
}
