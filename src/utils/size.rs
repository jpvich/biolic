//! Human-readable size parsing shared across modules.

use anyhow::{bail, Context, Result};

/// Parse a human size with an optional K/M/G suffix (base 1000), e.g. `1.5G`.
///
/// Used by any module that accepts a base/byte budget on the command line
/// (`sample --bases`, `head --bases`, `tail --bases`). Lives in the core so
/// modules share one implementation instead of importing each other's internals.
pub fn parse_size(s: &str) -> Result<u64> {
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
}
