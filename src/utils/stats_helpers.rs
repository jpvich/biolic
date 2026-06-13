//! Statistical helpers: N50, percentiles, distribution metrics.

/// Compute N50 from an unsorted slice of lengths.
///
/// N50 is defined as the length such that 50% of the total bases are in
/// reads of that length or longer. Algorithm: sort lengths descending,
/// accumulate until reaching half the total, return the current length.
///
/// Returns 0 for empty input.
pub fn n50(lengths: &[u64]) -> u64 {
    nx(lengths, 50)
}

/// Compute Nx (e.g., N50, N90) for arbitrary percentile x.
pub fn nx(lengths: &[u64], x: u8) -> u64 {
    if lengths.is_empty() || x == 0 || x > 100 {
        return 0;
    }
    let mut sorted: Vec<u64> = lengths.to_vec();
    sorted.sort_unstable_by(|a, b| b.cmp(a)); // descending

    let total: u64 = sorted.iter().sum();
    let threshold = (total as f64 * x as f64 / 100.0) as u64;

    let mut accumulated = 0u64;
    for &len in &sorted {
        accumulated += len;
        if accumulated >= threshold {
            return len;
        }
    }
    *sorted.last().unwrap_or(&0)
}

/// Median of a slice (computes via partial sort).
pub fn median(lengths: &[u64]) -> f64 {
    if lengths.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<u64> = lengths.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2] as f64
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) as f64 / 2.0
    }
}

/// Compute a percentile (0-100) using linear interpolation.
pub fn percentile(values: &[u64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<u64> = values.to_vec();
    sorted.sort_unstable();
    let n = sorted.len();
    let rank = (p / 100.0) * (n as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        sorted[lo] as f64
    } else {
        let weight = rank - lo as f64;
        sorted[lo] as f64 * (1.0 - weight) + sorted[hi] as f64 * weight
    }
}

/// Compute a percentile (0-100) over f64 values using linear interpolation.
pub fn percentile_f64(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted: Vec<f64> = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();
    let rank = (p / 100.0) * (n as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    if lo == hi {
        sorted[lo]
    } else {
        let weight = rank - lo as f64;
        sorted[lo] * (1.0 - weight) + sorted[hi] * weight
    }
}

/// Mean of a slice.
pub fn mean(values: &[u64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<u64>() as f64 / values.len() as f64
}

/// Min, max as a tuple. Returns (0, 0) for empty input.
pub fn min_max(values: &[u64]) -> (u64, u64) {
    if values.is_empty() {
        return (0, 0);
    }
    let mut min = u64::MAX;
    let mut max = 0u64;
    for &v in values {
        if v < min {
            min = v;
        }
        if v > max {
            max = v;
        }
    }
    (min, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_n50_canonical() {
        // Canonical N50 example: lengths [2, 2, 2, 3, 3, 4, 8, 8]
        // Total: 32, half: 16
        // Sorted desc: [8, 8, 4, 3, 3, 2, 2, 2]
        // Accumulated: 8, 16 (>=16, return 8)
        let lengths = vec![2, 2, 2, 3, 3, 4, 8, 8];
        assert_eq!(n50(&lengths), 8);
    }

    #[test]
    fn test_n50_simple() {
        // 10 reads of length 100. N50 = 100.
        let lengths = vec![100; 10];
        assert_eq!(n50(&lengths), 100);
    }

    #[test]
    fn test_n50_empty() {
        assert_eq!(n50(&[]), 0);
    }

    #[test]
    fn test_median() {
        assert_eq!(median(&[1, 2, 3]), 2.0);
        assert_eq!(median(&[1, 2, 3, 4]), 2.5);
        assert_eq!(median(&[]), 0.0);
    }

    #[test]
    fn test_min_max() {
        assert_eq!(min_max(&[5, 2, 8, 1, 9, 3]), (1, 9));
        assert_eq!(min_max(&[]), (0, 0));
    }
}
