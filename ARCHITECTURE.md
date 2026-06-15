# biolic Architecture

This document explains how biolic is built and how to extend it. It is the
reference for contributors. If you want to add a command, the section you need is
[The Module Contract](#the-module-contract).

## Philosophy: a platform that grows

biolic is a **stable core** plus a **growing catalog of modules**. The core
provides fixed contracts (record model, streaming I/O, output rendering, CLI);
every command is a self-contained module built against those contracts. New
capability arrives as new modules — without changing the core.

This is how biolic scales to many contributors: a module plugs into fixed
interfaces and cannot destabilize the engine. It is also how biolic stays
focused while remaining ambitious — breadth is earned by adding modules, not by
bloating the core.

## Scope

biolic is focused, not minimal and not everything.

| | |
|---|---|
| **Core domain** | `stats`, `count`, `filter`, `convert`, `sample`, `grep`, `head`/`tail`, plus biolic's own `qc` (interpretive QC) and `logs` (telemetry). |
| **Welcome as modules** | General fastx manipulation (`sort`, `rmdup`, `subseq`, `split`, `rename`, `replace`), indexing (`faidx`), search (`locate`, `amplicon`), and adjacent domains — added against the Module Contract, core or community. |
| **Out of scope (for now)** | Full aligners, assemblers, variant callers. biolic *interoperates* with them (reads BAM, streams via pipes) rather than reimplementing them. |

Capability grows outward in rings; each begins once the previous is solid:

1. **Long-read preprocessing** (now) — stats, count, filter (incl. trimming), convert, sample, grep, head/tail.
2. **General fastx operations** — seq/transform, subseq, sort, rmdup, split, pair, concat, translate.
3. **Interpretation & observability** — `qc`, `logs`.
4. **Adjacent domains & ecosystem** — alignment/assembly QC, annotation, Python bindings, reports (increasingly community-driven).

### How the modules cover the field

biolic groups **by operation** — one coherent verb per module — rather than
shipping a separate command for every micro-task. A compact module catalog
therefore covers the common ground of tools like `seqkit` and `seqtk`, while
staying teachable:

| Ring | Modules | Typical operations |
|---|---|---|
| 1 | `stats`, `count`, `filter`, `convert`, `sample`, `grep`, `head`/`tail` | summarize, filter by length/quality + end trimming, format conversion, subsample (incl. by target coverage), search, subset |
| 2 | `seq`/`transform`, `subseq`, `sort`, `rmdup`, `split`, `pair`, `concat`, `translate` | reverse-complement/case/rename/replace, region & window extraction, sorting, dedup, splitting, re-pairing, translation |
| 3 | `qc`, `logs` | interpretive QC, queryable execution history |

A single module folds several historically-separate commands (e.g. `convert`
covers FASTQ↔FASTA and tabular conversions; `seq` covers reverse-complement,
case, gap-stripping, rename, and replace) behind one verb and a set of flags.

## Layered design

```
╔══════════════════════════════════════════════════════╗
║  MODULE LAYER  (grows; core or community)             ║
║  stats · count · filter · convert · sample · grep ·   ║
║  head · tail · qc · logs · …future modules…           ║
║  each: clap Args + run(args, ctx) + Tabular result    ║
╚══════════════════════════════════════════════════════╝
                         │ depends on ▼ (one direction only)
┌──────────────────────────────────────────────────────┐
│  STABLE CORE  (the platform — fixed contracts)        │
│  • CLI & dispatch (clap) + interactive REPL           │
│  • Record model + streaming RecordReader              │
│  • I/O & format detection (noodles: FASTQ/FASTA/BAM;  │
│    flate2 gzip; niffler bz2/xz/zst behind a feature)  │
│  • Output layer: Tabular → human table / TSV / JSON   │
│  • Shared utils (quality, stats helpers, formatting)  │
│  • Telemetry (logs) + RunContext                      │
└──────────────────────────────────────────────────────┘
```

**Dependency rule: module → core only.** No module imports another module's
internals; shared logic belongs in the core (`utils`, `io`, `output`).

### Source layout

- `src/cli.rs` — top-level CLI and dispatch.
- `src/io/` — the `Record` model, streaming `RecordReader`, and format detection.
- `src/output/` — the output layer (`Tabular` → human table / TSV / JSON).
- `src/utils/` — shared helpers (quality scores, stats helpers, formatting).
- `src/modules/` — one file per command. Most contributions go here.
- `tests/integration/` — CLI integration tests against `tests/data/`.

`src/modules/stats.rs` and `src/modules/count.rs` are the reference modules.

## The Record model

A `Record` is one sequence (a FASTQ record, FASTA entry, or unaligned BAM
record) exposing name, sequence, optional quality, and length. Modules consume
records through a streaming `RecordReader`; the source format is abstracted
away. Adding a new **input format** means implementing a reader that yields
`Record`s — no module changes.

## The output layer

Modules never format output by hand. A result type derives
`serde::Serialize` and implements `output::Tabular`:

- `columns()` returns the column headers and alignment.
- `cells()` returns typed `Cell`s (`Int`, `Float`, `Percent`, `Text`).

The core then renders:

- **Human** — an aligned table (dynamic widths, numeric columns right-aligned,
  thousands separators on integers). Default on a TTY.
- **TSV** — header plus one raw row per item. Default when piped.
- **JSON** — a single object for one row, an array for several. Explicit `--json`.

Multi-input modules produce **one row per file** by default; `--combine`
aggregates into one row.

## The Module Contract

A module is the unit of contribution. Implement this and touch nothing else:

1. **An `Args` struct** deriving `clap::Args` (flags + inputs).
   - Per-command short flags; global flags are long-only.
   - For multiple input files, default to per-file output and provide `--combine`.
2. **`pub fn run(args: <Name>Args, ctx: &RunContext) -> anyhow::Result<()>`** —
   the only entry point the dispatcher calls. Keep it a **thin I/O wrapper**:
   it opens the reader/writer, drives the loop, and prints the summary — the
   logic lives in the pure core (next point).
3. **A pure, I/O-free core** that operates on `Record`s and never touches files,
   stdout, or stderr. Shape it to the module:
   - *Reporting* modules return data — `compute(...) -> Vec<T>` where `T` derives
     `serde::Serialize` and implements `output::Tabular` (e.g. `stats`, `count`);
     the core renders human/TSV/JSON for you.
   - *Transform* modules expose a per-record function — `apply(Record) ->
     Option<Record>` (`filter`) or `Result<Record>` (`convert`).
   - *Stream-stateful* algorithms expose a small type — e.g. `sample`'s
     `Reservoir` (offer records, take results).

   Keeping the core free of I/O makes it unit-testable in isolation **and** lets
   future Python bindings (PyO3/maturin, Ring 4) wrap the same core without the
   CLI — see [Python bindings](#python-bindings). Cores must not depend on
   `clap`; build them from plain fields (`Filter::from_args` is the bridge).
4. **Streaming** via `io::reader::open_input`. Keep memory O(1) in file size
   unless an algorithm provably needs more (document it). Record output goes
   through `io::writer` (`FastqWriter`/`FastaWriter`).
5. **Tests**: ≥1 integration test against `tests/data/small.*`, plus unit tests
   for the pure core (it is the easy, dependency-free thing to test).

A module **must not** import another module's internals, print progress to
stdout (use stderr), or read whole files into memory.

Wiring in a new command: add the `Args` variant to `src/cli.rs`, dispatch to
`run`, and optionally add its flags to the REPL completion table in
`src/repl.rs`.

## Python bindings

Python bindings (publishable to PyPI) are a Ring 4 goal, not yet built — but the
architecture is shaped for them now so adding them later is cheap:

- biolic is already a **library** (`src/lib.rs`), and module logic lives in
  **pure, I/O-free cores** (see contract point 3). Bindings wrap those cores;
  they do not re-implement anything.
- The intended path is **PyO3 + maturin** (likely a separate `biolic-py` crate
  with `crate-type = ["cdylib"]`), exposing the cores as Python functions and a
  `Record` type, with the heavy loops staying in Rust for near-native speed.
- **Performance**: wrapping pure cores adds no cost to the native CLI (Rust
  iterators are zero-cost; the binary is a separate artifact). From Python, keep
  per-record loops inside Rust (e.g. `biolic.filter(path, ...)` returns a summary
  or writes a file) for native speed; per-record iteration *into* Python carries
  the usual FFI cost and is opt-in.

The practical rule today: **do not fuse a module's logic into its I/O loop.**
Every transform must remain expressible over `Record`s independently of where
they come from or go.

## Conventions

- **stdout** is data only; **stderr** is progress and messages.
- Errors go to stderr as `error: <message>` (lowercase); exit non-zero on error.
- Don't average Phred scores arithmetically — use `utils::quality::mean_quality`.
- No platform-specific code paths without a `cfg` gate.
- The core contracts follow SemVer; breaking a public interface is a major bump.

See [`CONTRIBUTING.md`](./CONTRIBUTING.md) for the development workflow.
