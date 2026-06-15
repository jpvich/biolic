//! Nucleotide helpers shared across modules: reverse-complement and IUPAC
//! degenerate-base matching.

/// Complement of a single base, preserving case for `ACGT`, mapping `N`/`n` to
/// itself, and passing any other byte through unchanged.
#[inline]
fn complement(base: u8) -> u8 {
    match base {
        b'A' => b'T',
        b'C' => b'G',
        b'G' => b'C',
        b'T' => b'A',
        b'a' => b't',
        b'c' => b'g',
        b'g' => b'c',
        b't' => b'a',
        other => other, // N, U, IUPAC codes, gaps, etc. left as-is
    }
}

/// Reverse complement of a nucleotide sequence.
///
/// Complements `ACGT` (case-preserving) and reverses; unknown bytes (e.g. `N`,
/// IUPAC ambiguity codes) are complemented to themselves so length is preserved.
pub fn reverse_complement(seq: &[u8]) -> Vec<u8> {
    seq.iter().rev().map(|&b| complement(b)).collect()
}

/// Whether `base` satisfies an IUPAC `code`, case-insensitively.
///
/// `code` is an IUPAC nucleotide code (e.g. `N` = any, `R` = A/G, `Y` = C/T);
/// `base` is an observed nucleotide. `U` is treated as `T`. A code that is not a
/// recognized IUPAC letter only matches itself (case-folded).
pub fn iupac_matches(code: u8, base: u8) -> bool {
    let b = base.to_ascii_uppercase();
    // Normalize RNA uracil to thymine on both sides.
    let b = if b == b'U' { b'T' } else { b };
    let allowed: &[u8] = match code.to_ascii_uppercase() {
        b'A' => b"A",
        b'C' => b"C",
        b'G' => b"G",
        b'T' | b'U' => b"T",
        b'R' => b"AG",
        b'Y' => b"CT",
        b'S' => b"GC",
        b'W' => b"AT",
        b'K' => b"GT",
        b'M' => b"AC",
        b'B' => b"CGT",
        b'D' => b"AGT",
        b'H' => b"ACT",
        b'V' => b"ACG",
        b'N' => b"ACGT",
        // Not an IUPAC code: match the literal byte, case-folded.
        other => return b == other,
    };
    allowed.contains(&b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn revcomp_basic() {
        assert_eq!(reverse_complement(b"ACGT"), b"ACGT"); // palindrome
        assert_eq!(reverse_complement(b"AAAA"), b"TTTT");
        assert_eq!(reverse_complement(b"GATTACA"), b"TGTAATC");
    }

    #[test]
    fn revcomp_preserves_case_and_unknowns() {
        assert_eq!(reverse_complement(b"acgt"), b"acgt");
        assert_eq!(reverse_complement(b"ACGTN"), b"NACGT");
    }

    #[test]
    fn revcomp_round_trips() {
        let seq = b"ACGTACGTTGCANNAC";
        assert_eq!(reverse_complement(&reverse_complement(seq)), seq);
    }

    #[test]
    fn iupac_membership() {
        assert!(iupac_matches(b'N', b'A'));
        assert!(iupac_matches(b'N', b'g'));
        assert!(iupac_matches(b'R', b'A'));
        assert!(iupac_matches(b'R', b'G'));
        assert!(!iupac_matches(b'R', b'C'));
        assert!(iupac_matches(b'Y', b'T'));
        assert!(!iupac_matches(b'Y', b'A'));
        // Case-insensitive, U == T.
        assert!(iupac_matches(b'T', b'u'));
        assert!(iupac_matches(b'a', b'A'));
        // Concrete codes only match themselves.
        assert!(iupac_matches(b'A', b'A'));
        assert!(!iupac_matches(b'A', b'C'));
    }
}
