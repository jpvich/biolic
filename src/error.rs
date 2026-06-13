//! Error types for biolic.
//!
//! Most internal code uses `anyhow::Result` for ergonomics, but specific error
//! variants are useful at module boundaries and in the library API.

use std::path::PathBuf;
use thiserror::Error;

/// The top-level error type for biolic.
#[derive(Error, Debug)]
pub enum BiolicError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Unrecognized format for file: {0}")]
    UnrecognizedFormat(PathBuf),

    #[error("FASTQ parse error: {0}")]
    FastqParse(String),

    #[error("BAM parse error: {0}")]
    BamParse(String),

    #[error("Invalid quality score: {0}")]
    InvalidQuality(u8),

    #[error("Module not yet implemented: {0}")]
    NotImplemented(String),

    #[error("Invalid argument: {0}")]
    InvalidArgument(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
