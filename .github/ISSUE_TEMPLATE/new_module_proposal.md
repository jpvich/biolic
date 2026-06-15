---
name: New module proposal
about: Propose a new biolic subcommand/module
title: "[module] "
labels: module-proposal
---

> biolic grows through modules. Proposing one here lets us agree on scope and
> CLI shape **before** code, per the Module Contract in `ARCHITECTURE.md`.

## Module name and purpose

`biolic <name>` — one line on what it does.

## Why it belongs in biolic

Which ring does it fit (see the rings in `ARCHITECTURE.md`)? Is it long-read
relevant? What gap does it fill
that existing tools don't, or do awkwardly?

## Proposed CLI

```bash
biolic <name> [INPUTS]... [flags]
```

List the flags and what they do. Note any multi-file / `--combine` behavior.

## Output

What does it produce (filtered records? a stats table? a new file)? Which output
formats (human table / TSV / JSON)?

## Prior art

How do `seqkit` / `seqtk` / `samtools` / others do this? Link the reference.

## Streaming / memory

Can it run in O(1) memory, or does it inherently need to buffer? Explain.
