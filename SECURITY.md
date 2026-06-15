# Security Policy

biolic processes sequencing files (FASTQ, FASTA, unaligned BAM; plain or
gzipped) that often come from untrusted or third-party sources. We take the
safety of that parsing path seriously and welcome responsible reports.

## Supported versions

biolic is pre-1.0 and under active development. Security fixes are applied to
the latest released version on the `0.1.x` line and the `main` branch.

| Version | Supported |
|---------|-----------|
| 0.1.x   | ✅        |
| < 0.1   | ❌        |

Once 1.0 ships, this table will track the current and previous minor releases.

## Reporting a vulnerability

**Please do not open a public issue for a security vulnerability.** Public
issues disclose the problem before a fix is available.

Report privately through either channel:

1. **GitHub private vulnerability reporting (preferred).** Go to the
   repository's **Security** tab → **Report a vulnerability**. This opens a
   private advisory visible only to the maintainers and you.
2. **Email.** Send details to `jplicona@themaic.com` with `biolic security` in
   the subject line.

Please include, where possible:

- the biolic version (`biolic --version`) and how it was installed;
- the platform and Rust toolchain (`rustc --version`);
- a minimal input file or command that triggers the issue (a small crafted
  FASTQ/FASTA/BAM is ideal);
- the observed behavior (crash, hang, excessive memory, etc.) and what you
  expected.

## What to expect

- **Acknowledgement** within 3 business days.
- An initial **assessment** (severity, affected versions) within 7 days.
- We aim to release a fix or mitigation within **30 days** of confirmation,
  sooner for high-severity issues.
- We practice **coordinated disclosure**: once a fix is released, a security
  advisory is published. Reporters are credited unless they prefer to remain
  anonymous.

## Scope

In scope — issues in biolic's own code, for example:

- memory-safety problems or crashes triggered by malformed or adversarial input;
- panics that abort the process on input that should be rejected gracefully
  (a denial-of-service vector for pipelines);
- unbounded memory or CPU on crafted input (e.g. decompression "bombs",
  pathological records) that breaks the streaming, constant-memory guarantee;
- writing outside the intended output path, or other unexpected filesystem or
  process behavior driven by input or arguments.

Out of scope:

- vulnerabilities in third-party dependencies — please report those upstream;
  if biolic's use of a dependency is what exposes the issue, tell us too;
- incorrect biological results that are not a safety problem — those are
  regular bugs; open a normal issue;
- attacks that require an already-compromised machine or modified binary.

## Project security practices

- biolic's own source contains **no `unsafe` code**; memory safety is provided
  by the Rust compiler. Sequence parsing goes through the well-maintained
  `noodles` crate rather than hand-rolled parsers.
- The toolkit is **streaming-first** (constant memory regardless of file size),
  which limits memory-exhaustion surface by design.
- Continuous integration runs the test suite, `cargo fmt`, and
  `cargo clippy -D warnings` on every change; dependency freshness is tracked
  publicly via deps.rs.
- The committed `Cargo.lock` pins exact dependency versions for reproducible,
  auditable builds.

Thank you for helping keep biolic and its users safe.
