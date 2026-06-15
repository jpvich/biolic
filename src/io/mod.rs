//! Input/output abstractions.
//!
//! The reader subsystem provides a unified interface to read records from any
//! supported format (FASTQ, FASTQ.gz, FASTA, BAM). All modules consume `Record`
//! instances from a `RecordReader` trait object, isolating them from format
//! details.

pub mod reader;
pub mod writer;

pub use reader::{open_reader, Format, RecordReader};
pub use writer::{FastaWriter, FastqWriter, RecordWriter};
