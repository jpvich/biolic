//! Regenerate `tests/data/small.bam` — a tiny unaligned BAM fixture.
//!
//! There is no samtools dependency in this project, so the fixture is generated
//! with noodles. Run with:
//!
//! ```bash
//! cargo run --example gen_bam
//! ```
//!
//! Contents: 3 unaligned reads, 8 bp each (24 bp total).

use std::fs::File;

use noodles::sam::alignment::io::Write as _;
use noodles::sam::alignment::record::Flags;
use noodles::sam::alignment::RecordBuf;

fn main() -> std::io::Result<()> {
    let header = noodles::sam::Header::default();
    let file = File::create("tests/data/small.bam")?;
    let mut writer = noodles::bam::io::Writer::new(file);
    writer.write_header(&header)?;

    let reads: [(&str, &[u8], u8); 3] = [
        ("read1", b"ACGTACGT", 40),
        ("read2", b"GGGGCCCC", 30),
        ("read3", b"ACGTNNNN", 20),
    ];

    for (name, seq, q) in reads {
        let record = RecordBuf::builder()
            .set_flags(Flags::UNMAPPED)
            .set_name(name)
            .set_sequence(seq.to_vec().into())
            .set_quality_scores(vec![q; seq.len()].into())
            .build();
        writer.write_alignment_record(&header, &record)?;
    }

    writer.try_finish()?;
    println!("wrote tests/data/small.bam (3 reads, 24 bp)");
    Ok(())
}
