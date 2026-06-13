//! The unified `Record` type.
//!
//! All modules in biolic operate on `Record` instances, regardless of whether
//! the underlying file is FASTQ, FASTA, or BAM. This abstraction allows new
//! formats to be added by implementing only the reader, without modifying
//! every module.

/// A single sequencing read.
///
/// The `qual` field is `None` for FASTA records (no quality scores).
/// The `tags` field is populated only for BAM records that carry auxiliary tags.
#[derive(Debug, Clone)]
pub struct Record {
    /// Read name / identifier.
    pub id: Vec<u8>,

    /// Nucleotide sequence (uppercase ASCII: A, C, G, T, N).
    pub seq: Vec<u8>,

    /// Per-base quality scores in Phred+33 encoding.
    /// `None` for FASTA records.
    pub qual: Option<Vec<u8>>,

    /// Optional comment / description from the FASTQ/FASTA header.
    pub desc: Option<Vec<u8>>,
}

impl Record {
    /// Construct a new record.
    pub fn new(id: Vec<u8>, seq: Vec<u8>, qual: Option<Vec<u8>>) -> Self {
        Self {
            id,
            seq,
            qual,
            desc: None,
        }
    }

    /// Construct a record with a description.
    pub fn with_desc(
        id: Vec<u8>,
        seq: Vec<u8>,
        qual: Option<Vec<u8>>,
        desc: Option<Vec<u8>>,
    ) -> Self {
        Self {
            id,
            seq,
            qual,
            desc,
        }
    }

    /// Length of the sequence in bases.
    #[inline]
    pub fn len(&self) -> usize {
        self.seq.len()
    }

    /// Whether the record is empty (zero-length sequence).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.seq.is_empty()
    }

    /// Whether the record has quality scores (i.e., is FASTQ-like).
    #[inline]
    pub fn has_quality(&self) -> bool {
        self.qual.is_some()
    }

    /// Count of each nucleotide in the sequence.
    /// Returns (A, C, G, T, N, other).
    pub fn base_counts(&self) -> (u64, u64, u64, u64, u64, u64) {
        let mut a = 0u64;
        let mut c = 0u64;
        let mut g = 0u64;
        let mut t = 0u64;
        let mut n = 0u64;
        let mut other = 0u64;

        for &b in &self.seq {
            match b {
                b'A' | b'a' => a += 1,
                b'C' | b'c' => c += 1,
                b'G' | b'g' => g += 1,
                b'T' | b't' => t += 1,
                b'N' | b'n' => n += 1,
                _ => other += 1,
            }
        }
        (a, c, g, t, n, other)
    }

    /// GC content as a fraction of (G+C) / (A+C+G+T).
    /// Returns 0.0 if no valid bases.
    pub fn gc_content(&self) -> f64 {
        let (a, c, g, t, _n, _other) = self.base_counts();
        let acgt = a + c + g + t;
        if acgt == 0 {
            return 0.0;
        }
        (c + g) as f64 / acgt as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_basic() {
        let r = Record::new(b"read1".to_vec(), b"ACGT".to_vec(), Some(b"IIII".to_vec()));
        assert_eq!(r.len(), 4);
        assert!(!r.is_empty());
        assert!(r.has_quality());
    }

    #[test]
    fn test_base_counts() {
        let r = Record::new(b"r1".to_vec(), b"AACCGGTTN".to_vec(), None);
        let (a, c, g, t, n, other) = r.base_counts();
        assert_eq!(a, 2);
        assert_eq!(c, 2);
        assert_eq!(g, 2);
        assert_eq!(t, 2);
        assert_eq!(n, 1);
        assert_eq!(other, 0);
    }

    #[test]
    fn test_gc_content() {
        let r = Record::new(b"r1".to_vec(), b"GCGC".to_vec(), None);
        assert!((r.gc_content() - 1.0).abs() < 1e-9);

        let r2 = Record::new(b"r2".to_vec(), b"ATAT".to_vec(), None);
        assert!((r2.gc_content() - 0.0).abs() < 1e-9);

        let r3 = Record::new(b"r3".to_vec(), b"ACGT".to_vec(), None);
        assert!((r3.gc_content() - 0.5).abs() < 1e-9);
    }
}
