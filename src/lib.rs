//! biolic: Bioinformatics Integrated Operations Library for IO & Computation.
//!
//! A modular bioinformatics toolkit in Rust, designed for fast, memory-efficient
//! processing of DNA sequencing data,
//! with particular focus on long-read sequencing from PacBio HiFi and Oxford Nanopore.
//!
//! # Modules
//!
//! - [`cli`]: Command-line interface definition and dispatch.
//! - [`io`]: File reading, writing, and format detection.
//! - [`record`]: The unified `Record` type abstracting FASTQ, FASTA, and BAM.
//! - [`modules`]: The actual processing modules (stats, filter, count, etc.).
//! - [`output`]: Output formatting (human, JSON, TSV).
//! - [`utils`]: Shared utility functions (quality, statistics).
//! - [`error`]: Error types.
//!
//! # Example
//!
//! ```no_run
//! use biolic::io::reader::open_reader;
//! use biolic::record::Record;
//!
//! let mut reader = open_reader("reads.fastq.gz").unwrap();
//! let mut total = 0;
//! while let Some(record) = reader.next_record().unwrap() {
//!     total += record.seq.len();
//! }
//! println!("Total bases: {}", total);
//! ```

pub mod banner;
pub mod cli;
pub mod error;
pub mod io;
pub mod modules;
pub mod output;
pub mod record;
pub mod repl;
pub mod utils;

pub use error::BiolicError;
pub use record::Record;
