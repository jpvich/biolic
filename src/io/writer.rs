//! Unified record writer.
//!
//! The counterpart to [`crate::io::reader`]: modules that emit records (e.g.
//! `filter`, and later `convert`) write them through a `RecordWriter` without
//! knowing the concrete output format. FASTA is written for quality-less
//! records; FASTQ for records that carry quality scores.

use std::io::Write;

use anyhow::{anyhow, Result};

use crate::record::Record;

/// A streaming writer of [`Record`]s.
pub trait RecordWriter {
    /// Write a single record.
    fn write_record(&mut self, rec: &Record) -> Result<()>;

    /// Flush any buffered output. Call once at the end.
    fn flush(&mut self) -> Result<()>;
}

/// Write a header line: `prefix`(`@`/`>`) + id, plus an optional description.
fn write_header<W: Write>(w: &mut W, prefix: u8, id: &[u8], desc: Option<&[u8]>) -> Result<()> {
    w.write_all(&[prefix])?;
    w.write_all(id)?;
    if let Some(d) = desc {
        if !d.is_empty() {
            w.write_all(b" ")?;
            w.write_all(d)?;
        }
    }
    w.write_all(b"\n")?;
    Ok(())
}

/// Writes records as FASTQ (`@id`/seq/`+`/qual).
pub struct FastqWriter<W: Write> {
    inner: W,
}

impl<W: Write> FastqWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }
}

impl<W: Write> RecordWriter for FastqWriter<W> {
    fn write_record(&mut self, rec: &Record) -> Result<()> {
        let qual = rec.qual.as_ref().ok_or_else(|| {
            anyhow!(
                "cannot write FASTQ: record '{}' has no quality scores",
                String::from_utf8_lossy(&rec.id)
            )
        })?;
        write_header(&mut self.inner, b'@', &rec.id, rec.desc.as_deref())?;
        self.inner.write_all(&rec.seq)?;
        self.inner.write_all(b"\n+\n")?;
        self.inner.write_all(qual)?;
        self.inner.write_all(b"\n")?;
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        self.inner.flush()?;
        Ok(())
    }
}

/// Writes records as FASTA (`>id`/seq). Quality scores, if any, are dropped.
pub struct FastaWriter<W: Write> {
    inner: W,
}

impl<W: Write> FastaWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner }
    }
}

impl<W: Write> RecordWriter for FastaWriter<W> {
    fn write_record(&mut self, rec: &Record) -> Result<()> {
        write_header(&mut self.inner, b'>', &rec.id, rec.desc.as_deref())?;
        self.inner.write_all(&rec.seq)?;
        self.inner.write_all(b"\n")?;
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        self.inner.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fastq_rec() -> Record {
        Record::with_desc(
            b"read1".to_vec(),
            b"ACGT".to_vec(),
            Some(b"IIII".to_vec()),
            Some(b"len=4".to_vec()),
        )
    }

    #[test]
    fn fastq_writer_roundtrips_format() {
        let mut buf = Vec::new();
        {
            let mut w = FastqWriter::new(&mut buf);
            w.write_record(&fastq_rec()).unwrap();
            w.flush().unwrap();
        }
        assert_eq!(buf, b"@read1 len=4\nACGT\n+\nIIII\n");
    }

    #[test]
    fn fasta_writer_drops_quality() {
        let mut buf = Vec::new();
        {
            let mut w = FastaWriter::new(&mut buf);
            w.write_record(&fastq_rec()).unwrap();
            w.flush().unwrap();
        }
        assert_eq!(buf, b">read1 len=4\nACGT\n");
    }

    #[test]
    fn fastq_writer_errors_without_quality() {
        let rec = Record::new(b"r".to_vec(), b"ACGT".to_vec(), None);
        let mut buf = Vec::new();
        let mut w = FastqWriter::new(&mut buf);
        assert!(w.write_record(&rec).is_err());
    }
}
