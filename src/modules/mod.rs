//! Module dispatch.
//!
//! Each subcommand of biolic is a module here. To add a new module:
//! 1. Create a new file (e.g., `myop.rs`) in this directory.
//! 2. Add `pub mod myop;` here.
//! 3. Implement `pub struct MyopArgs` with `clap::Args` derive.
//! 4. Implement `pub fn run(args: MyopArgs, ctx: &RunContext) -> Result<()>`.
//! 5. Add the variant to `Commands` in `src/cli.rs`.

pub mod convert;
pub mod count;
pub mod filter;
pub mod grep;
pub mod head;
pub mod logs;
pub mod qc;
pub mod sample;
pub mod stats;
pub mod tail;
