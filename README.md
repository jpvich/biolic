```text
██████╗  ██╗ ██████╗   ██╗      ██╗ ██████╗
██╔══██╗ ██║ ██╔═══██╗ ██║      ██║ ██╔════╝
██████╔╝ ██║ ██║   ██║ ██║      ██║ ██║
██╔══██╗ ██║ ██║   ██║ ██║      ██║ ██║
██████╔╝ ██║ ╚██████╔╝ ███████╗ ██║ ╚██████╗
╚═════╝  ╚═╝ ╚═════╝   ╚══════╝ ╚═╝ ╚═════╝
```

# biolic

**Bioinformatics Integrated Operations Library for IO & Computation** — a fast, modular, streaming bioinformatics toolkit in Rust for sequencing data (FASTQ/FASTA/BAM), built as a platform that grows through modules, with particularly strong support for long reads (Oxford Nanopore, PacBio HiFi).

[![CI](https://github.com/jpvich/biolic/actions/workflows/ci.yml/badge.svg)](https://github.com/jpvich/biolic/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/biolic.svg)](https://crates.io/crates/biolic)
[![Downloads](https://img.shields.io/crates/d/biolic.svg)](https://crates.io/crates/biolic)
[![Docs.rs](https://docs.rs/biolic/badge.svg)](https://docs.rs/biolic)
[![Rust 1.89+](https://img.shields.io/badge/rust-1.89%2B-orange.svg)](https://www.rust-lang.org/)
[![Dependencies](https://deps.rs/repo/github/jpvich/biolic/status.svg)](https://deps.rs/repo/github/jpvich/biolic)
[![License: MIT/Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

## What is biolic?

biolic is a **general-purpose** bioinformatics toolkit: it unifies the most common
operations on sequencing data — statistics, filtering, conversion, sampling, search,
and quality control — into a single fast, memory-efficient binary. It works on FASTQ,
FASTA, and unaligned BAM (DNA, RNA, or protein), and is built as a **platform that grows
through modules**, so new capability is added without bloating the core. Its support for
long reads (Oxford Nanopore, PacBio HiFi) is particularly strong.

Today, a bioinformatician juggles `samtools`, `nanoq`, `chopper`, `seqtk`, `rasusa`,
`seqkit`, and more — each with a different CLI, install method, and quirks. biolic
replaces them with one binary, one consistent CLI, and streaming-first performance.

## Status

**Phase 1, in active development (v0.1.1).** Working today:
- `biolic stats` — N50/N90, mean/median length, quality, GC, and length + quality percentiles (`--extended`); one row per file by default, `--combine` to aggregate
- `biolic count` — fast read and base counting; per-file or `--combine`
- `biolic filter` — keep/drop reads by length, mean quality, GC, and N content, with fixed crops (`--headcrop`/`--tailcrop`) and quality trimming (`--trim-quality`)
- `biolic convert` — BAM→FASTQ/FASTA, FASTQ↔FASTA, and gzip (de)compression (format inferred from the `-o` extension, or `--to` for stdout)
- `biolic sample` — subsample by count (`-n`), proportion (`-p`), total bases (`--bases`), or target coverage (`--coverage`/`--genome-size`); reproducible with `--seed`
- `biolic head` / `biolic tail` — first/last N reads (`-n`) or bases (`--bases`); `head` stops reading early, `tail` buffers only a bounded suffix
- `biolic grep` — search by sequence or name: exact (`-p`), multi-pattern (`-f`), regex (`--regex`), approximate (`--mismatches`), IUPAC degenerate (`-d`), and reverse-complement (`--both-strands`); `-v` to invert, case-insensitive by default
- **Input**: FASTQ, FASTA, and unaligned BAM — plain or gzipped, from files or stdin (format auto-detected)
- **Output**: aligned columnar table (human), `--json`, or `--tsv` (one row per file)
- **Interactive REPL**: run `biolic` with no arguments for a `biolic>` prompt with tab-completion
- **Shell completions**: `biolic completions <bash|zsh|fish>`

Coming next:
- `biolic qc` — adaptive, interpretive QC (mixture models, anomaly detection, threshold recommendations)
- `biolic logs` — queryable execution history

## Installation

### From source

```bash
git clone https://github.com/jpvich/biolic
cd biolic
cargo build --release
./target/release/biolic --help
```

### From crates.io

```bash
cargo install biolic
```

### From Bioconda (once published)

```bash
conda install -c bioconda biolic
```

## Usage

```bash
# Compute statistics
biolic stats reads.fastq.gz

# JSON output for pipelines
biolic stats reads.fastq.gz --json

# Fast counting
biolic count reads.fastq.gz

# Filter by length and quality, trimming low-quality ends
biolic filter -q 10 -l 500 --trim-quality 12 reads.fastq.gz > clean.fastq

# Convert BAM to gzipped FASTQ (format inferred from the extension)
biolic convert reads.bam -o reads.fastq.gz

# Subsample to ~30x coverage of a 5 Mbp genome, reproducibly
biolic sample --coverage 30 --genome-size 5M --seed 42 reads.fastq.gz > sub.fastq

# Peek at the first 100 reads, or the last 1 Mbp
biolic head -n 100 reads.fastq.gz
biolic tail --bases 1M reads.fastq.gz

# Find reads containing an adapter on either strand, allowing 1 mismatch
biolic grep -p AGATCGGAAGAGC --both-strands --mismatches 1 reads.fastq.gz

# Match a degenerate primer (IUPAC codes) in read sequences
biolic grep -d -p GGNTGG reads.fastq.gz

# Pipe through tools
cat reads.fastq | biolic filter -q 10 | biolic stats
```

Example output:

```
File:              reads.fastq.gz
Reads:             1,234,567
Total bases:       9,876,543,210
Min length:        52
Max length:        87,432
Mean length:       8,001.2
Median length:     6,543.0
N50:               12,345
N90:               2,109
Mean quality:      28.54
Bases above Q10:   99.82%
Bases above Q20:   94.31%
Bases above Q30:   71.15%
GC content:        42.18%
```

## Design principles

1. **Streaming first**: constant memory regardless of file size.
2. **Single binary**: zero runtime dependencies, no Python, no Docker.
3. **Format-general, long-read strong**: works on FASTQ/FASTA/BAM (DNA, RNA, or
   protein); especially capable on long reads (N50/N90, native BAM, per-position analysis).
4. **Modern UX**: JSON output, automatic format detection, predictable CLI.
5. **A platform that grows**: each operation is an independent module against a
   stable core, so new capability is added without touching the engine.

## Supported formats

| Format | Read | Write |
|---|---|---|
| FASTQ | ✓ | ✓ |
| FASTQ.gz (gzip) | ✓ | ✓ |
| FASTQ.bz2 / .xz / .zst | ✓ *(build with `--features extra-compression`)* | — |
| FASTA / FASTA.gz | ✓ | planned |
| BAM (unaligned) | ✓ | planned |

## Performance

biolic is designed to be:
- Comparable in speed to nanoq (current Rust reference for stats/filter)
- Faster than seqkit (avoids Go GC overhead)
- Dramatically faster than Python tools (NanoFilt, NanoStat)
- Lower memory than all of the above through streaming

Benchmarks against `nanoq`, `chopper`, `seqkit`, and `seqtk` will be published with v0.1
release. See `docs/benchmarks.md`.

## Roadmap

biolic is built in phases: a streaming preprocessing toolkit first (stats, count,
filter, convert, sample, grep, head/tail), then an adaptive QC module and a
queryable execution history, with Python bindings and Bioconda packaging to follow.

## Contributing

Contributions are welcome — biolic is built to grow through **modules**. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the workflow and
[ARCHITECTURE.md](ARCHITECTURE.md) for the design and the Module Contract (how to
add a command). Please open an issue before major changes; there are templates
for bugs, features, and new-module proposals.

The project uses standard Rust tooling:
- `cargo fmt` before commits
- `cargo clippy` must pass
- `cargo test` must pass

## Security

biolic parses files that may come from untrusted sources. To report a
vulnerability, see [SECURITY.md](SECURITY.md) — please use private reporting
rather than a public issue.

## License

Dual-licensed under MIT or Apache-2.0, at your option. See [LICENSE-MIT](LICENSE-MIT)
and [LICENSE-APACHE](LICENSE-APACHE).

## Citation

If you use biolic in research, please cite (forthcoming):

```
jpvich (2026). biolic: A modular bioinformatics toolkit in Rust.
[Software paper venue forthcoming]
```
