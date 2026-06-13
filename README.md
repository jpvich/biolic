# biolic

**Bioinformatics Integrated Operations Library for IO & Computation** — a modular, streaming bioinformatics toolkit in Rust for processing long-read sequencing data.

[![CI](https://github.com/jpvich/biolic/actions/workflows/ci.yml/badge.svg)](https://github.com/jpvich/biolic/actions/workflows/ci.yml)
[![License: MIT/Apache-2.0](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

## What is biolic?

biolic unifies the most common operations on sequencing data — statistics, filtering,
conversion, sampling, search, and quality control — into a single fast, memory-efficient
binary with first-class support for PacBio HiFi and Oxford Nanopore long reads.

Today, a bioinformatician working with long-read data must combine `samtools`, `nanoq`,
`chopper`, `seqtk`, `rasusa`, and `seqkit` — each with different CLIs, install methods,
and quirks. biolic replaces all of them with one binary, one consistent CLI, and
streaming-first performance.

## Status

**Phase 1, in active development (v0.1.1).** Working today:
- `biolic stats` — N50/N90, mean/median length, quality, GC, and length + quality percentiles (`--extended`); multi-file aggregation
- `biolic count` — fast read and base counting
- **Input**: FASTQ, FASTA, and unaligned BAM — plain or gzipped, from files or stdin (format auto-detected)
- **Output**: human-readable, `--json`, or `--tsv`
- **Interactive REPL**: run `biolic` with no arguments for a `biolic>` prompt with tab-completion
- **Shell completions**: `biolic completions <bash|zsh|fish>`

Coming next:
- `biolic filter`, `biolic convert`, `biolic sample`, `biolic grep`, `biolic head`, `biolic tail`
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

# Pipe through tools
cat reads.fastq | biolic stats
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
3. **Long-read native**: N50, native BAM input, per-position analysis.
4. **Modern UX**: JSON output, automatic format detection, predictable CLI.
5. **Modular**: each operation is an independent module, easy to add new ones.

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

Contributions are welcome. Please open an issue before submitting major changes.

The project uses standard Rust tooling:
- `cargo fmt` before commits
- `cargo clippy` must pass
- `cargo test` must pass

## License

Dual-licensed under MIT or Apache-2.0, at your option. See [LICENSE-MIT](LICENSE-MIT)
and [LICENSE-APACHE](LICENSE-APACHE).

## Citation

If you use biolic in research, please cite (forthcoming):

```
jpvich (2026). biolic: A modular bioinformatics toolkit in Rust.
[Software paper venue forthcoming]
```
