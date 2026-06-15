# Contributing to biolic

Thanks for your interest in biolic! biolic is designed to grow through
**modules**: a small, stable core plus a catalog of self-contained operations.
Most contributions are a new module or an improvement to an existing one, and
the architecture is built so you can add capability without touching the engine.

## Ground rules

- Be respectful and constructive in issues, PRs, and reviews.
- Discuss non-trivial changes in an issue **before** writing code, so scope and
  CLI shape are agreed up front.
- All work targets the `dev` branch (`main` is the release branch).

## Development setup

```bash
git clone https://github.com/jpvich/biolic
cd biolic
cargo build
cargo test
cargo run -- stats tests/data/small.fastq
```

Before opening a PR, make sure these pass:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Project layout

- `src/cli.rs` — top-level CLI and dispatch.
- `src/io/` — the streaming `Record` model and format detection (FASTQ/FASTA/BAM,
  gzip; bz2/xz/zst behind the `extra-compression` feature).
- `src/output/` — the shared output layer (`Tabular` → human table / TSV / JSON).
- `src/utils/` — shared helpers (quality scores, stats helpers, formatting).
- `src/modules/` — one file per command. **This is where most contributions go.**
- `tests/integration/` — CLI integration tests run against `tests/data/`.

`src/modules/stats.rs` and `src/modules/count.rs` are the reference
implementations — copy their shape.

## Adding a module (the Module Contract)

See [`ARCHITECTURE.md`](./ARCHITECTURE.md) for the full picture. In short, a
module plugs into fixed core contracts and touches nothing else. Implement:

1. **An `Args` struct** deriving `clap::Args` for the command's flags/inputs.
   - Use per-command short flags; global flags are long-only.
   - For commands that take multiple input files, follow the per-file default
     and provide `--combine` to aggregate (see `stats`).
2. **`pub fn run(args: <Name>Args, ctx: &RunContext) -> anyhow::Result<()>`** —
   the only entry point the dispatcher calls, kept as a **thin I/O wrapper**.
3. **A pure, I/O-free core** holding the logic (no files/stdout/stderr, no
   `clap`). Reporting modules return a result type that derives
   `serde::Serialize` and implements `output::Tabular` (`columns()` + typed
   `Cell`s) — the core renders human/TSV/JSON, never hand-rolled. Transform
   modules expose a per-record `apply(Record) -> Option/Result<Record>`
   (see `filter`, `convert`); stream algorithms expose a small type (see
   `sample`'s `Reservoir`). This keeps logic testable and Python-bindable.
4. **Streaming**: read records via `io::reader::open_input`. Keep memory O(1) in
   file size unless an algorithm provably needs more (document it if so).
5. **Tests**: at least one integration test against `tests/data/small.*`, plus
   unit tests for non-trivial logic.

A module **must not** import another module's internals, print progress to
stdout (use stderr), or read whole files into memory. Shared logic belongs in
the core (`utils`, `io`, `output`).

Wiring a new command in: add the `Args` variant to `src/cli.rs`, dispatch to
`run`, and (optionally) add its flags to the REPL completion table in
`src/repl.rs`.

## Conventions

- **stdout** is for data only; **stderr** is for progress and messages.
- Default output is the human table on a TTY, TSV when piped; `--json` is
  explicit. One row per input file by default.
- Errors go to stderr as `error: <message>` (lowercase). Exit non-zero on error.
- Don't average Phred scores arithmetically — use `utils::quality::mean_quality`.
- No platform-specific code paths without a `cfg` gate.

## Pull requests

- Keep PRs focused: one logical change.
- Include tests and update `README.md` / `CHANGELOG.md` where relevant.
- Ensure CI is green (lint + tests + MSRV across supported targets).

## Licensing

biolic is dual-licensed **MIT OR Apache-2.0**. By contributing, you agree your
contributions are licensed under the same terms (inbound = outbound). No CLA is
required.

See [`ARCHITECTURE.md`](./ARCHITECTURE.md) for the full architecture, scope, and
the Module Contract.
