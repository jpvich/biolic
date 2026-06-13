//! Startup banner, shown when `biolic` is run with no subcommand.
//!
//! Always written to stderr (never stdout) so it cannot corrupt piped data.
//! Colors are emitted only when stderr is a terminal.

use std::io::{IsTerminal, Write};

const LOGO: &str = "██████╗  ██╗ ██████╗   ██╗      ██╗ ██████╗
██╔══██╗ ██║ ██╔═══██╗ ██║      ██║ ██╔════╝
██████╔╝ ██║ ██║   ██║ ██║      ██║ ██║
██╔══██╗ ██║ ██║   ██║ ██║      ██║ ██║
██████╔╝ ██║ ╚██████╔╝ ███████╗ ██║ ╚██████╗
╚═════╝  ╚═╝ ╚═════╝   ╚══════╝ ╚═╝ ╚═════╝";

const TAGLINE: &str = "Bioinformatics Integrated Operations Library for IO & Computation";

/// Print the banner to stderr.
pub fn print_banner() {
    let _ = write_banner(&mut std::io::stderr());
}

fn write_banner(w: &mut impl Write) -> std::io::Result<()> {
    let (cyan, bold, dim, reset) = if std::io::stderr().is_terminal() {
        ("\x1b[36m", "\x1b[1m", "\x1b[2m", "\x1b[0m")
    } else {
        ("", "", "", "")
    };

    writeln!(w)?;
    for line in LOGO.lines() {
        writeln!(w, "{cyan}{bold}{line}{reset}")?;
    }
    writeln!(w)?;
    writeln!(w, "{bold}{TAGLINE}{reset}")?;
    writeln!(w)?;
    writeln!(
        w,
        "  {dim}Version:{reset}     {}",
        env!("CARGO_PKG_VERSION")
    )?;
    writeln!(
        w,
        "  {dim}Build date:{reset}  {}",
        env!("BIOLIC_BUILD_DATE")
    )?;
    writeln!(
        w,
        "  {dim}Repository:{reset}  {}",
        env!("CARGO_PKG_REPOSITORY")
    )?;
    writeln!(
        w,
        "  {dim}License:{reset}     {}",
        env!("CARGO_PKG_LICENSE")
    )?;
    writeln!(w)?;
    writeln!(w, "Run `biolic --help` to see available commands.")?;
    Ok(())
}
