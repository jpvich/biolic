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
fn stats_multi_file_is_per_file_by_default() {
    // Two distinct files -> a JSON array with one object each (not aggregated).
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["stats", FASTQ, FASTA, "--json"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("["))
        .stdout(predicate::str::contains("\"read_count\": 5")) // small.fastq
        .stdout(predicate::str::contains("\"read_count\": 3")); // small.fasta
}

#[test]
fn stats_combine_aggregates() {
    // --combine sums all inputs into one row: FASTQ twice doubles both.
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["stats", FASTQ, FASTQ, "--combine", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"read_count\": 10"))
        .stdout(predicate::str::contains("\"total_bases\": 412"))
        .stdout(predicate::str::contains("\"file\": \"total\""));
}

#[test]
fn count_multi_file_is_per_file_by_default() {
    // Two files -> JSON array with one object each.
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["count", FASTQ, FASTA, "--json"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("["))
        .stdout(predicate::str::contains("\"reads\": 5"))
        .stdout(predicate::str::contains("\"reads\": 3"));
}

#[test]
fn count_combine_aggregates() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["count", FASTQ, FASTQ, "--combine", "--json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"reads\": 10"))
        .stdout(predicate::str::contains("\"bases\": 412"));
}

#[test]
fn stats_multi_file_tsv_has_one_row_per_file() {
    // TSV: a single header line, then one data row per input file.
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["stats", FASTQ, FASTA, "--tsv"])
        .assert()
        .success()
        .stdout(predicate::function(|s: &str| {
            let lines: Vec<&str> = s.lines().collect();
            lines.len() == 3 && lines[0].starts_with("file\t")
        }));
}

#[test]
fn stats_basename_strips_directory_in_human_table() {
    // Human (aligned) output via an explicit non-TTY run still honors --basename;
    // force human-ish check via TSV file column instead.
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["stats", FASTQ, "--basename", "--tsv"])
        .assert()
        .success()
        .stdout(predicate::str::contains("small.fastq"))
        .stdout(predicate::str::contains("tests/data/small.fastq").not());
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
    // `grep` is still a stub; it must fail cleanly rather than panic.
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["grep", "ACGT", FASTQ])
        .assert()
        .failure();
}

#[test]
fn sample_count_keeps_exactly_n() {
    let out = Command::cargo_bin("biolic")
        .unwrap()
        .args(["sample", "-n", "2", "--seed", "1", FASTQ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert_eq!(text.lines().count(), 8, "2 FASTQ records = 8 lines");
    assert!(text.starts_with('@'), "FASTQ output starts with '@'");
}

#[test]
fn sample_proportion_one_keeps_all() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["sample", "-p", "1.0", FASTQ])
        .assert()
        .success()
        .stderr(predicate::str::contains("kept 5 / 5"));
}

#[test]
fn sample_proportion_zero_keeps_none() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["sample", "-p", "0.0", FASTQ])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn sample_is_reproducible_with_seed() {
    let run = || {
        Command::cargo_bin("biolic")
            .unwrap()
            .args(["sample", "-n", "3", "--seed", "42", FASTQ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone()
    };
    assert_eq!(run(), run(), "same seed must give identical output");
}

#[test]
fn sample_bases_requires_a_file_not_stdin() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["sample", "--bases", "100"])
        .write_stdin(fs::read(FASTQ).unwrap())
        .assert()
        .failure()
        .stderr(predicate::str::contains("require a file input"));
}

#[test]
fn sample_coverage_runs_on_a_file() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["sample", "--coverage", "2", "--genome-size", "100", FASTQ])
        .assert()
        .success();
}

#[test]
fn convert_fastq_to_fasta_strips_quality() {
    let out = Command::cargo_bin("biolic")
        .unwrap()
        .args(["convert", FASTQ, "--to", "fasta"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.starts_with('>'), "FASTA output must start with '>'");
    assert!(!text.contains("\n+\n"), "FASTA must not contain a '+' line");
    assert_eq!(text.matches('>').count(), 5);
}

#[test]
fn convert_bam_to_fastq() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["convert", BAM, "--to", "fastq"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("@"));
}

#[test]
fn convert_fasta_to_fastq_requires_fake_quality() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["convert", FASTA, "--to", "fastq"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--fake-quality"));
}

#[test]
fn convert_fasta_to_fastq_with_fake_quality() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["convert", FASTA, "--to", "fastq", "--fake-quality", "30"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("@"));
}

#[test]
fn convert_gzip_output_roundtrips() {
    // Write a gzipped FASTQ to the per-test temp dir, then read it back.
    let out_path = format!("{}/convert_out.fastq.gz", env!("CARGO_TARGET_TMPDIR"));
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["convert", FASTQ, "-o", &out_path])
        .assert()
        .success();
    // The gzip stream must be well-formed: counting it yields the 5 input reads.
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["count", "--json", &out_path])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"reads\": 5"));
}

#[test]
fn convert_bz2_output_is_rejected() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["convert", FASTQ, "-o", "out.fastq.bz2"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not supported"));
}

#[test]
fn filter_min_length_keeps_subset() {
    // small.fastq has 5 reads of varying length; only the 100 bp read is >= 50.
    let out = Command::cargo_bin("biolic")
        .unwrap()
        .args(["filter", "-l", "50", FASTQ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    // Valid FASTQ: 4 lines per record, exactly one record kept.
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 4, "expected exactly one FASTQ record");
    assert!(lines[0].starts_with('@'));
    assert_eq!(lines[2], "+");
    assert_eq!(lines[1].len(), lines[3].len(), "seq and qual length differ");
}

#[test]
fn filter_summary_goes_to_stderr() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["filter", "-l", "50", FASTQ])
        .assert()
        .success()
        .stderr(predicate::str::contains("kept 1 / 5"));
}

#[test]
fn filter_fasta_input_yields_fasta_output() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["filter", "-l", "12", FASTA])
        .assert()
        .success()
        .stdout(predicate::str::starts_with(">"));
}

#[test]
fn filter_trim_quality_shortens_reads() {
    // Trimming low-quality ends must reduce total output bytes vs no trimming.
    let untrimmed = Command::cargo_bin("biolic")
        .unwrap()
        .args(["filter", FASTQ])
        .assert()
        .success()
        .get_output()
        .stdout
        .len();
    let trimmed = Command::cargo_bin("biolic")
        .unwrap()
        .args(["filter", "--trim-quality", "20", FASTQ])
        .assert()
        .success()
        .get_output()
        .stdout
        .len();
    assert!(
        trimmed < untrimmed,
        "trimmed output ({trimmed}) should be smaller than untrimmed ({untrimmed})"
    );
}

#[test]
fn head_takes_first_n_reads() {
    let out = Command::cargo_bin("biolic")
        .unwrap()
        .args(["head", "-n", "2", FASTQ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert_eq!(text.lines().count(), 8, "2 FASTQ records = 8 lines");
    assert!(text.contains("@read1"));
    assert!(text.contains("@read2"));
    assert!(!text.contains("@read3"), "must stop after the first 2");
}

#[test]
fn head_default_is_ten_reads() {
    // small.fastq has only 5 reads; the default of 10 yields all of them.
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["head", FASTQ])
        .assert()
        .success()
        .stderr(predicate::str::contains("wrote 5 records"));
}

#[test]
fn head_bases_stops_early_whole_records() {
    // Each read in small.fastq is short; --bases 1 reaches the target after the
    // first whole record and stops.
    let out = Command::cargo_bin("biolic")
        .unwrap()
        .args(["head", "--bases", "1", FASTQ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("@read1"));
    assert!(
        !text.contains("@read2"),
        "one whole record satisfies --bases 1"
    );
}

#[test]
fn head_preserves_fasta_format() {
    let out = Command::cargo_bin("biolic")
        .unwrap()
        .args(["head", "-n", "2", FASTA])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(text.starts_with('>'), "FASTA output starts with '>'");
    assert!(!text.contains('+'), "no FASTQ separator in FASTA output");
    assert!(text.contains(">seq1"));
    assert!(text.contains(">seq2"));
    assert!(!text.contains(">seq3"));
}

#[test]
fn head_reads_and_bases_conflict() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["head", "-n", "2", "--bases", "100", FASTQ])
        .assert()
        .failure();
}

#[test]
fn tail_takes_last_n_reads() {
    let out = Command::cargo_bin("biolic")
        .unwrap()
        .args(["tail", "-n", "2", FASTQ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert_eq!(text.lines().count(), 8, "2 FASTQ records = 8 lines");
    assert!(text.contains("@read4"));
    assert!(text.contains("@read5"));
    assert!(!text.contains("@read3"), "only the last 2");
}

#[test]
fn tail_more_than_available_keeps_all() {
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["tail", "-n", "100", FASTQ])
        .assert()
        .success()
        .stderr(predicate::str::contains("wrote 5 records"));
}

#[test]
fn tail_reads_from_stdin() {
    let data = fs::read(FASTQ).unwrap();
    Command::cargo_bin("biolic")
        .unwrap()
        .args(["tail", "-n", "1"])
        .write_stdin(data)
        .assert()
        .success()
        .stdout(predicate::str::contains("@read5"));
}
