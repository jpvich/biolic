//! Integration tests for the biolic CLI.
//!
//! These run the compiled binary against `tests/data/small.fastq` (5 reads,
//! 206 total bases). Note: `assert_cmd` pipes stdout, so the default output
//! format is TSV (not human-readable), per biolic's TTY-detection convention.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

const FASTQ: &str = "tests/data/small.fastq";
const FASTQ_GZ: &str = "tests/data/small.fastq.gz";
const FASTA: &str = "tests/data/small.fasta";
const BAM: &str = "tests/data/small.bam";

#[test]
fn stats_default_output_is_tsv_when_piped() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["stats", FASTQ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("file\t"))
        .stdout(predicate::str::contains("n50"));
}

#[test]
fn stats_json_reports_five_reads() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["stats", FASTQ, "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"read_count\": 5"))
        .stdout(predicate::str::contains("\"total_bases\": 206"))
        .stdout(predicate::str::contains("\"n50\""));
}

#[test]
fn count_json_reports_five_reads() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["count", FASTQ, "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"reads\": 5"))
        .stdout(predicate::str::contains("\"bases\": 206"));
}

#[test]
fn stats_reads_gzip_input() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["stats", FASTQ_GZ, "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"read_count\": 5"))
        .stdout(predicate::str::contains("\"total_bases\": 206"));
}

#[test]
fn stats_reads_from_stdin() {
    let data = fs::read(FASTQ).unwrap();
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["stats", "-", "--json"])
        .write_stdin(data)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"read_count\": 5"));
}

#[test]
fn stats_reads_gzip_from_stdin() {
    let data = fs::read(FASTQ_GZ).unwrap();
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["stats", "-", "--json"])
        .write_stdin(data)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"read_count\": 5"));
}

#[test]
fn stats_reads_fasta() {
    // FASTA: 3 sequences, 12 + 16 + 12 = 40 bases, no quality.
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["stats", FASTA, "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"read_count\": 3"))
        .stdout(predicate::str::contains("\"total_bases\": 40"));
}

#[test]
fn count_reads_fasta() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["count", FASTA, "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"reads\": 3"))
        .stdout(predicate::str::contains("\"bases\": 40"));
}

#[test]
fn stats_reads_unaligned_bam() {
    // Unaligned BAM: 3 reads, 8 bp each (24 bp total), with quality scores.
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["stats", BAM, "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"read_count\": 3"))
        .stdout(predicate::str::contains("\"total_bases\": 24"));
}

#[test]
fn count_reads_unaligned_bam() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["count", BAM, "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"reads\": 3"))
        .stdout(predicate::str::contains("\"bases\": 24"));
}

#[test]
fn stats_extended_includes_percentiles() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["stats", FASTQ, "--extended", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"length_percentiles\""))
        .stdout(predicate::str::contains("\"quality_percentiles\""))
        .stdout(predicate::str::contains("\"p50\""));
}

#[test]
fn stats_extended_fasta_has_length_but_no_quality_percentiles() {
    // FASTA has no quality, so quality_percentiles is omitted.
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["stats", FASTA, "--extended", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"length_percentiles\""))
        .stdout(predicate::str::contains("\"quality_percentiles\"").not());
}

#[test]
fn stats_multi_file_aggregates() {
    // small.fastq is 5 reads / 206 bases; passing it twice doubles both.
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["stats", FASTQ, FASTQ, "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"read_count\": 10"))
        .stdout(predicate::str::contains("\"total_bases\": 412"));
}

#[test]
fn count_multi_file_aggregates() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["count", FASTQ, FASTQ, "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"reads\": 10"))
        .stdout(predicate::str::contains("\"bases\": 412"));
}

#[test]
fn stats_no_args_reads_stdin() {
    let data = fs::read(FASTQ).unwrap();
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["stats", "--json"])
        .write_stdin(data)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"read_count\": 5"));
}

#[test]
fn stats_reads_fasta_from_stdin() {
    let data = fs::read(FASTA).unwrap();
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["stats", "-", "--json"])
        .write_stdin(data)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"read_count\": 3"))
        .stdout(predicate::str::contains("\"total_bases\": 40"));
}

#[test]
fn stats_reads_bam_from_stdin() {
    let data = fs::read(BAM).unwrap();
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["stats", "-", "--json"])
        .write_stdin(data)
        .assert()
        .success()
        .stdout(predicate::str::contains("\"read_count\": 3"))
        .stdout(predicate::str::contains("\"total_bases\": 24"));
}

#[test]
fn no_command_shows_banner_on_stderr() {
    Command::cargo_bin("biolic")
        .unwrap()
        .assert()
        .success()
        // Banner goes to stderr; stdout stays empty so pipelines are unaffected.
        .stdout(predicate::str::is_empty())
        .stderr(predicate::str::contains(
            "Bioinformatics Integrated Operations Library for IO & Computation",
        ))
        .stderr(predicate::str::contains("Version:"));
}

#[test]
fn completions_zsh_generates_script() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["completions", "zsh"])
        .assert()
        .success()
        .stdout(predicate::str::contains("biolic"));
}

#[test]
fn help_works() {
    Command::cargo_bin("biolic")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("biolic"));
}

#[test]
fn unimplemented_module_errors_cleanly() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["filter", FASTQ])
        .assert()
        .failure();
}
