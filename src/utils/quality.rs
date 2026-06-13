//! Quality score conversions and averaging.
//!
//! IMPORTANT: Phred quality scores cannot be averaged arithmetically. They
//! must be converted to error probabilities first, then averaged, then
//! converted back. Many tools get this wrong; biolic does it correctly.

/// Convert a Phred+33 ASCII quality character to an error probability.
///
/// Q = -10 * log10(P)  =>  P = 10^(-Q/10)
#[inline]
pub fn phred_to_prob(phred_char: u8) -> f64 {
    let q = (phred_char.saturating_sub(33)) as f64;
    10f64.powf(-q / 10.0)
}

/// Convert an error probability back to a Phred score (numeric, not ASCII).
#[inline]
pub fn prob_to_phred(prob: f64) -> f64 {
    if prob <= 0.0 {
        // Cap at Q60 for numerical stability
        return 60.0;
    }
    -10.0 * prob.log10()
}

/// Compute the correct mean Phred quality for a slice of quality scores.
///
/// Averages in probability space, then converts back to Phred.
pub fn mean_quality(qual: &[u8]) -> f64 {
    if qual.is_empty() {
        return 0.0;
    }
    let sum_prob: f64 = qual.iter().map(|&c| phred_to_prob(c)).sum();
    let mean_prob = sum_prob / qual.len() as f64;
    prob_to_phred(mean_prob)
}

/// Compute the simple (incorrect but commonly used) arithmetic mean of Phred scores.
///
/// Provided for compatibility with tools that compute it this way.
pub fn arithmetic_mean_quality(qual: &[u8]) -> f64 {
    if qual.is_empty() {
        return 0.0;
    }
    let sum: u64 = qual.iter().map(|&c| (c.saturating_sub(33)) as u64).sum();
    sum as f64 / qual.len() as f64
}

/// Count bases with quality at or above a threshold.
pub fn count_above_threshold(qual: &[u8], threshold: u8) -> usize {
    let threshold_char = threshold + 33;
    qual.iter().filter(|&&c| c >= threshold_char).count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phred_to_prob() {
        // Q0 = P 1.0
        assert!((phred_to_prob(b'!') - 1.0).abs() < 1e-9);
        // Q10 = P 0.1
        assert!((phred_to_prob(b'+') - 0.1).abs() < 1e-9);
        // Q20 = P 0.01
        assert!((phred_to_prob(b'5') - 0.01).abs() < 1e-9);
        // Q30 = P 0.001
        assert!((phred_to_prob(b'?') - 0.001).abs() < 1e-9);
    }

    #[test]
    fn test_mean_quality_uniform() {
        // All Q20 -> mean Q20
        let qual = vec![b'5'; 100];
        let m = mean_quality(&qual);
        assert!((m - 20.0).abs() < 0.01);
    }

    #[test]
    fn test_mean_quality_differs_from_arithmetic() {
        // Mix of Q40 and Q10 should be much closer to Q10 than the
        // arithmetic mean of Q25 would suggest.
        let mut qual = vec![b'I'; 50]; // Q40
        qual.extend(vec![b'+'; 50]); // Q10
        let prob_mean = mean_quality(&qual);
        let arith_mean = arithmetic_mean_quality(&qual);
        // Arithmetic average is 25
        assert!((arith_mean - 25.0).abs() < 0.01);
        // Probability average should be ~13 (dominated by lower quality bases)
        assert!(prob_mean < 14.0);
        assert!(prob_mean > 12.0);
    }
}
