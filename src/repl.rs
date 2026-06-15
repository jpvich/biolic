//! Interactive REPL, entered when `biolic` runs with no subcommand on a TTY.
//!
//! A `biolic> ` prompt where commands are typed and run in a loop (one-shot mode
//! is unchanged). Tab opens a completion dropdown over subcommands, per-command
//! flags, and file paths; history is persisted and offers inline hints.
//!
//! See decision D8 in `biolic_plan.md`.

use std::borrow::Cow;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use clap::Parser;
use reedline::{
    default_emacs_keybindings, ColumnarMenu, Completer, DefaultHinter, Emacs, FileBackedHistory,
    History, KeyCode, KeyModifiers, MenuBuilder, Prompt, PromptEditMode, PromptHistorySearch,
    Reedline, ReedlineEvent, ReedlineMenu, Signal, Span, Suggestion,
};

use crate::cli::{Cli, RunContext};

/// Subcommands offered in the REPL, plus REPL-only built-ins.
const COMMANDS: &[(&str, &str)] = &[
    ("stats", "Compute summary statistics"),
    ("count", "Count reads and bases"),
    ("filter", "Filter and trim reads by length, quality, GC, N"),
    ("convert", "Convert between formats (FASTQ/FASTA/BAM, gzip)"),
    (
        "sample",
        "Subsample reads by count, proportion, or coverage",
    ),
    ("grep", "Search reads by sequence or name"),
    ("head", "Extract the first N reads or bases"),
    ("tail", "Extract the last N reads or bases"),
    ("qc", "Adaptive QC (not yet implemented)"),
    ("logs", "Execution logs (not yet implemented)"),
    ("completions", "Generate a shell completion script"),
    ("help", "List commands"),
    ("exit", "Leave the interactive session"),
];

/// Flags suggested after a given command.
fn flags_for(command: &str) -> &'static [(&'static str, &'static str)] {
    match command {
        "stats" => &[
            ("--json", "JSON output"),
            ("--tsv", "TSV output"),
            ("--extended", "Length & quality percentiles"),
            ("-e", "Length & quality percentiles"),
            ("--combine", "Aggregate all inputs into one row"),
            ("--basename", "Show only the file basename"),
            ("-b", "Show only the file basename"),
            ("--help", "Show help"),
        ],
        "count" => &[
            ("--json", "JSON output"),
            ("--tsv", "TSV output"),
            ("--combine", "Aggregate all inputs into one row"),
            ("--basename", "Show only the file basename"),
            ("-b", "Show only the file basename"),
            ("--help", "Show help"),
        ],
        "filter" => &[
            ("--min-length", "Minimum read length"),
            ("-l", "Minimum read length"),
            ("--max-length", "Maximum read length"),
            ("-m", "Maximum read length"),
            ("--min-quality", "Minimum mean quality"),
            ("-q", "Minimum mean quality"),
            ("--max-n", "Maximum N proportion"),
            ("--min-gc", "Minimum GC content"),
            ("--max-gc", "Maximum GC content"),
            ("--headcrop", "Trim N bases from start"),
            ("--tailcrop", "Trim N bases from end"),
            ("--trim-quality", "Quality-trim ends to best segment"),
            ("--help", "Show help"),
        ],
        "convert" => &[
            ("-o", "Output file (format inferred from extension)"),
            ("--output", "Output file (format inferred from extension)"),
            ("--to", "Target format for stdout (fastq|fasta)"),
            ("--fake-quality", "Quality to assign for FASTA→FASTQ"),
            ("--help", "Show help"),
        ],
        "sample" => &[
            ("-n", "Keep exactly N reads"),
            ("--count", "Keep exactly N reads"),
            ("-p", "Keep each read with probability"),
            ("--proportion", "Keep each read with probability"),
            ("--bases", "Sample down to ~SIZE bases (1G, 500M)"),
            ("--coverage", "Target coverage (needs --genome-size)"),
            ("--genome-size", "Genome size for --coverage"),
            ("--seed", "Random seed for reproducibility"),
            ("--help", "Show help"),
        ],
        "grep" => &[
            ("-p", "Single search pattern"),
            ("--pattern", "Single search pattern"),
            ("-f", "File of patterns (one per line)"),
            ("--patterns-file", "File of patterns (one per line)"),
            ("--regex", "Patterns are regular expressions"),
            ("--mismatches", "Allow up to N substitutions"),
            ("-d", "IUPAC degenerate codes (N, R, Y, …)"),
            ("--degenerate", "IUPAC degenerate codes (N, R, Y, …)"),
            ("-n", "Search read names, not sequences"),
            ("--in-name", "Search read names, not sequences"),
            ("--both-strands", "Also match the reverse complement"),
            ("-v", "Output reads that do NOT match"),
            ("--invert", "Output reads that do NOT match"),
            ("--case-sensitive", "Match case-sensitively"),
            ("--help", "Show help"),
        ],
        "head" | "tail" => &[
            ("-n", "Number of reads (default 10)"),
            ("--reads", "Number of reads (default 10)"),
            ("--bases", "Number of bases (K/M/G suffixes)"),
            ("--help", "Show help"),
        ],
        "completions" => &[("--help", "Show help")],
        _ => &[("--help", "Show help")],
    }
}

/// Run the interactive session. Returns when the user exits.
pub fn run_repl(ctx: &RunContext) -> Result<()> {
    eprintln!(
        "Interactive mode. Type a command (e.g. `stats reads.fastq --json`), \
         press Tab to complete, `help` to list commands, or `exit` to quit.\n"
    );

    let mut keybindings = default_emacs_keybindings();
    keybindings.add_binding(
        KeyModifiers::NONE,
        KeyCode::Tab,
        ReedlineEvent::UntilFound(vec![
            ReedlineEvent::Menu("completion_menu".to_string()),
            ReedlineEvent::MenuNext,
        ]),
    );
    let edit_mode = Box::new(Emacs::new(keybindings));
    let menu = ColumnarMenu::default().with_name("completion_menu");

    let mut line_editor = Reedline::create()
        .with_completer(Box::new(BiolicCompleter))
        .with_menu(ReedlineMenu::EngineCompleter(Box::new(menu)))
        .with_hinter(Box::new(DefaultHinter::default()))
        .with_edit_mode(edit_mode);

    if let Some(history) = history() {
        line_editor = line_editor.with_history(history);
    }

    let prompt = BiolicPrompt;

    loop {
        match line_editor.read_line(&prompt) {
            Ok(Signal::Success(line)) => {
                let line = line.trim();
                match line {
                    "" => continue,
                    "exit" | "quit" => break,
                    "help" => {
                        print_help();
                        continue;
                    }
                    _ => {
                        if let Err(e) = run_line(line, ctx) {
                            eprintln!("error: {e:#}");
                        }
                    }
                }
            }
            Ok(Signal::CtrlC) => continue,
            Ok(Signal::CtrlD) => break,
            // `Signal` is #[non_exhaustive]; ignore any future variants.
            Ok(_) => continue,
            Err(e) => {
                eprintln!("error: {e}");
                break;
            }
        }
    }
    Ok(())
}

/// Parse one typed line as a biolic command and dispatch it.
fn run_line(line: &str, ctx: &RunContext) -> Result<()> {
    let tokens =
        shlex::split(line).ok_or_else(|| anyhow!("could not parse input (unbalanced quotes?)"))?;
    if tokens.is_empty() {
        return Ok(());
    }

    let mut argv = Vec::with_capacity(tokens.len() + 1);
    argv.push("biolic".to_string());
    argv.extend(tokens);

    match Cli::try_parse_from(&argv) {
        Ok(cli) => {
            if let Some(command) = cli.command {
                crate::cli::dispatch(command, ctx)?;
            }
            Ok(())
        }
        // Show clap's formatted error/help, but stay in the session.
        Err(e) => {
            let _ = e.print();
            Ok(())
        }
    }
}

fn print_help() {
    println!("Commands:");
    for (name, desc) in COMMANDS {
        println!("  {name:<12} {desc}");
    }
    println!("\nType `<command> --help` for command-specific options.");
}

/// Persisted REPL history at `~/.biolic/repl_history.txt`, if HOME is available.
fn history() -> Option<Box<dyn History>> {
    let home = std::env::var_os("HOME")?;
    let dir = PathBuf::from(home).join(".biolic");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("repl_history.txt");
    FileBackedHistory::with_file(1000, path)
        .ok()
        .map(|h| Box::new(h) as Box<dyn History>)
}

/// The `biolic> ` prompt.
struct BiolicPrompt;

impl Prompt for BiolicPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed("biolic")
    }
    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }
    fn render_prompt_indicator(&self, _edit_mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("> ")
    }
    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed("... ")
    }
    fn render_prompt_history_search_indicator(&self, _s: PromptHistorySearch) -> Cow<'_, str> {
        Cow::Borrowed("(search) ")
    }
}

/// Completer for subcommands, per-command flags, and file paths.
struct BiolicCompleter;

impl Completer for BiolicCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<Suggestion> {
        let pos = pos.min(line.len());
        let head = &line[..pos];

        // Byte offset where the word under the cursor begins.
        let word_start = head
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace())
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        let current = &line[word_start..pos];
        let span = Span::new(word_start, pos);

        // Anything before the current word (trimmed) tells us the position.
        let before = head[..word_start].trim();

        if before.is_empty() {
            // Completing the command word.
            return COMMANDS
                .iter()
                .filter(|(name, _)| name.starts_with(current))
                .map(|(name, desc)| suggestion(name, Some(desc), span, true))
                .collect();
        }

        let command = before.split_whitespace().next().unwrap_or("");
        if current.starts_with('-') {
            flags_for(command)
                .iter()
                .filter(|(flag, _)| flag.starts_with(current))
                .map(|(flag, desc)| suggestion(flag, Some(desc), span, true))
                .collect()
        } else {
            complete_path(current, span)
        }
    }
}

fn suggestion(
    value: &str,
    description: Option<&str>,
    span: Span,
    append_whitespace: bool,
) -> Suggestion {
    Suggestion {
        value: value.to_string(),
        description: description.map(str::to_string),
        style: None,
        extra: None,
        span,
        append_whitespace,
        display_override: None,
        match_indices: None,
    }
}

/// Filesystem path completion for the current word.
fn complete_path(current: &str, span: Span) -> Vec<Suggestion> {
    let (dir, prefix) = match current.rfind('/') {
        Some(i) => (&current[..=i], &current[i + 1..]),
        None => ("", current),
    };
    let read_dir = if dir.is_empty() { "." } else { dir };

    let entries = match std::fs::read_dir(read_dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut out: Vec<Suggestion> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(prefix) {
            continue;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let mut value = format!("{dir}{name}");
        if is_dir {
            value.push('/');
        }
        out.push(suggestion(&value, None, span, !is_dir));
    }
    out.sort_by(|a, b| a.value.cmp(&b.value));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_command_prefix() {
        let mut c = BiolicCompleter;
        let s = c.complete("st", 2);
        assert!(s.iter().any(|x| x.value == "stats"));
        assert!(!s.iter().any(|x| x.value == "count")); // doesn't start with "st"
    }

    #[test]
    fn completes_flags_after_command() {
        let mut c = BiolicCompleter;
        let s = c.complete("stats --js", "stats --js".len());
        assert!(s.iter().any(|x| x.value == "--json"));
    }

    #[test]
    fn completes_paths_for_argument() {
        let mut c = BiolicCompleter;
        // Completing a path argument should surface the tests/ directory.
        let s = c.complete("stats te", "stats te".len());
        assert!(s.iter().any(|x| x.value.starts_with("te")));
    }
}
