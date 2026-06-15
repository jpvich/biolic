//! Output formatting (human-readable columnar tables, JSON, TSV).
//!
//! Modules produce a list of rows (typically one per input file). A row type
//! implements [`Tabular`], declaring its columns and per-row [`Cell`] values.
//! The renderer turns a slice of rows into:
//! - **Human**: an aligned table (numeric columns right-aligned, thousands
//!   separators on integers), shown by default on a TTY.
//! - **TSV**: a header line plus one raw row per item, shown when piped.
//! - **JSON**: a single object for one row, an array for several (via serde).

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
        if stdout_is_tty() {
            OutputFormat::Human
        } else {
            OutputFormat::Tsv
        }
    }

    /// Resolve from the common `--json` / `--tsv` flag pair (else auto).
    pub fn from_flags(json: bool, tsv: bool) -> Self {
        if json {
            OutputFormat::Json
        } else if tsv {
            OutputFormat::Tsv
        } else {
            OutputFormat::auto()
        }
    }
}

fn stdout_is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

/// Column alignment in the human-readable table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
}

/// A table column: a fixed header and an alignment for its cells.
#[derive(Debug, Clone, Copy)]
pub struct Column {
    pub header: &'static str,
    pub align: Align,
}

impl Column {
    pub const fn left(header: &'static str) -> Self {
        Column {
            header,
            align: Align::Left,
        }
    }
    pub const fn right(header: &'static str) -> Self {
        Column {
            header,
            align: Align::Right,
        }
    }
}

/// A single table cell. The variant carries the value's *type* so the renderer
/// can format it differently for humans (commas, `%`) vs. TSV (raw).
#[derive(Debug, Clone)]
pub enum Cell {
    /// Free text (e.g. a file name or format label).
    Text(String),
    /// An integer count; human output gets thousands separators.
    Int(u64),
    /// A float with a fixed number of decimal places.
    Float(f64, usize),
    /// A percentage; human output appends `%`, TSV emits the bare number.
    Percent(f64),
}

impl Cell {
    /// Rendered form for the aligned human table.
    fn human(&self) -> String {
        match self {
            Cell::Text(s) => s.clone(),
            Cell::Int(n) => fmt_commas(*n),
            Cell::Float(v, p) => format!("{v:.*}", p),
            Cell::Percent(v) => format!("{v:.2}%"),
        }
    }

    /// Rendered form for machine-readable TSV (no separators, no `%`).
    fn tsv(&self) -> String {
        match self {
            Cell::Text(s) => s.clone(),
            Cell::Int(n) => n.to_string(),
            Cell::Float(v, p) => format!("{v:.*}", p),
            Cell::Percent(v) => format!("{v:.2}"),
        }
    }

    /// Whether this cell type is numeric (right-aligned in human output).
    fn is_numeric(&self) -> bool {
        !matches!(self, Cell::Text(_))
    }
}

/// A type that can be rendered as a row in a table.
///
/// JSON output is provided automatically via [`serde::Serialize`].
pub trait Tabular {
    /// Column definitions (headers + alignment). Must match `cells()` length.
    fn columns(&self) -> Vec<Column>;
    /// This row's cells, in the same order as `columns()`.
    fn cells(&self) -> Vec<Cell>;
}

/// Write a list of tabular rows to stdout in the chosen format.
///
/// JSON emits a single object when there is exactly one row (back-compatible
/// with single-file output) and a JSON array when there are several.
pub fn write_rows<T: Tabular + Serialize>(rows: &[T], format: OutputFormat) -> Result<()> {
    let mut out = std::io::stdout();
    match format {
        OutputFormat::Json => {
            let json = if rows.len() == 1 {
                serde_json::to_string_pretty(&rows[0])?
            } else {
                serde_json::to_string_pretty(rows)?
            };
            println!("{json}");
        }
        OutputFormat::Tsv => render_tsv(rows, &mut out)?,
        OutputFormat::Human => render_human(rows, &mut out)?,
    }
    Ok(())
}

/// Render rows as a header line plus one tab-joined raw row each.
fn render_tsv<T: Tabular>(rows: &[T], w: &mut dyn std::io::Write) -> Result<()> {
    let columns = match rows.first() {
        Some(r) => r.columns(),
        None => return Ok(()),
    };
    let header: Vec<&str> = columns.iter().map(|c| c.header).collect();
    writeln!(w, "{}", header.join("\t"))?;
    for row in rows {
        let cells: Vec<String> = row.cells().iter().map(Cell::tsv).collect();
        writeln!(w, "{}", cells.join("\t"))?;
    }
    Ok(())
}

/// Render rows as an aligned table with dynamic column widths.
fn render_human<T: Tabular>(rows: &[T], w: &mut dyn std::io::Write) -> Result<()> {
    let columns = match rows.first() {
        Some(r) => r.columns(),
        None => return Ok(()),
    };

    // Pre-render every cell, then compute each column's width from its header
    // and its widest value.
    let rendered: Vec<Vec<String>> = rows
        .iter()
        .map(|r| r.cells().iter().map(Cell::human).collect())
        .collect();
    let numeric: Vec<bool> = rows
        .first()
        .map(|r| r.cells().iter().map(Cell::is_numeric).collect())
        .unwrap_or_default();

    let ncols = columns.len();
    let mut widths: Vec<usize> = columns.iter().map(|c| c.header.chars().count()).collect();
    for row in &rendered {
        for (i, cell) in row.iter().enumerate().take(ncols) {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }

    // Header.
    let mut line = String::new();
    for (i, col) in columns.iter().enumerate() {
        if i > 0 {
            line.push_str("  ");
        }
        line.push_str(&pad(col.header, widths[i], col.align));
    }
    writeln!(w, "{}", line.trim_end())?;

    // Data rows.
    for row in &rendered {
        let mut line = String::new();
        for i in 0..ncols {
            if i > 0 {
                line.push_str("  ");
            }
            let empty = String::new();
            let value = row.get(i).unwrap_or(&empty);
            // Numeric columns right-align regardless of declared alignment so
            // digits line up; text columns use the declared alignment.
            let align = if *numeric.get(i).unwrap_or(&false) {
                Align::Right
            } else {
                columns[i].align
            };
            line.push_str(&pad(value, widths[i], align));
        }
        writeln!(w, "{}", line.trim_end())?;
    }
    Ok(())
}

/// Pad `s` to `width` columns with the given alignment.
fn pad(s: &str, width: usize, align: Align) -> String {
    let len = s.chars().count();
    if len >= width {
        return s.to_string();
    }
    let fill = " ".repeat(width - len);
    match align {
        Align::Left => format!("{s}{fill}"),
        Align::Right => format!("{fill}{s}"),
    }
}

/// Format an integer with thousands separators, e.g. `1234567` -> `1,234,567`.
pub fn fmt_commas(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Row {
        name: &'static str,
        count: u64,
        pct: f64,
    }
    impl Tabular for Row {
        fn columns(&self) -> Vec<Column> {
            vec![
                Column::left("file"),
                Column::right("reads"),
                Column::right("gc"),
            ]
        }
        fn cells(&self) -> Vec<Cell> {
            vec![
                Cell::Text(self.name.to_string()),
                Cell::Int(self.count),
                Cell::Percent(self.pct),
            ]
        }
    }

    fn render(rows: &[Row], f: fn(&[Row], &mut dyn std::io::Write) -> Result<()>) -> String {
        let mut buf = Vec::new();
        f(rows, &mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn commas() {
        assert_eq!(fmt_commas(0), "0");
        assert_eq!(fmt_commas(999), "999");
        assert_eq!(fmt_commas(1_000), "1,000");
        assert_eq!(fmt_commas(1_234_567), "1,234,567");
    }

    #[test]
    fn human_table_aligns_and_separates() {
        let rows = vec![
            Row {
                name: "a.fastq",
                count: 1_234_567,
                pct: 42.5,
            },
            Row {
                name: "b.fq",
                count: 5,
                pct: 7.0,
            },
        ];
        let out = render(&rows, render_human);
        // Thousands separator present, percent sign present.
        assert!(out.contains("1,234,567"));
        assert!(out.contains("42.50%"));
        // Header present.
        assert!(out.lines().next().unwrap().contains("file"));
        // Numeric column right-aligned: the small "5" is padded to the width of
        // "1,234,567", so its line has leading spaces before the 5.
        let row_b = out.lines().find(|l| l.contains("b.fq")).unwrap();
        assert!(row_b.contains("        5"));
    }

    #[test]
    fn tsv_is_raw() {
        let rows = vec![Row {
            name: "a.fastq",
            count: 1_234_567,
            pct: 42.5,
        }];
        let out = render(&rows, render_tsv);
        let mut lines = out.lines();
        assert_eq!(lines.next().unwrap(), "file\treads\tgc");
        // No commas, no percent sign in TSV.
        assert_eq!(lines.next().unwrap(), "a.fastq\t1234567\t42.50");
    }
}
