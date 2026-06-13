//! Output formatting (human-readable, JSON, TSV).

use anyhow::Result;
use serde::Serialize;

/// Output format selected by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
    Tsv,
}

impl OutputFormat {
    /// Detect default format based on whether stdout is a terminal.
    pub fn auto() -> Self {
        if atty_isatty() {
            OutputFormat::Human
        } else {
            OutputFormat::Tsv
        }
    }
}

/// Best-effort TTY detection without an extra dependency.
fn atty_isatty() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

/// Write a serializable value to stdout in the chosen format.
pub fn write<T: Serialize + HumanDisplay>(value: &T, format: OutputFormat) -> Result<()> {
    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(value)?;
            println!("{}", json);
        }
        OutputFormat::Tsv => {
            value.write_tsv(&mut std::io::stdout())?;
        }
        OutputFormat::Human => {
            value.write_human(&mut std::io::stdout())?;
        }
    }
    Ok(())
}

/// Trait for types that can render themselves as human-readable text and TSV.
///
/// JSON output is provided automatically via `serde::Serialize`.
pub trait HumanDisplay {
    fn write_human(&self, w: &mut dyn std::io::Write) -> Result<()>;
    fn write_tsv(&self, w: &mut dyn std::io::Write) -> Result<()>;
}
