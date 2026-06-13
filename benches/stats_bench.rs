//! Benchmarks for the stats module.
//!
//! Run with: `cargo bench --bench stats_bench`

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn placeholder_bench(c: &mut Criterion) {
    c.bench_function("placeholder", |b| {
        b.iter(|| {
            // TODO: Add real benchmarks once tests/data has larger files
            black_box(2 + 2)
        })
    });
}

criterion_group!(benches, placeholder_bench);
criterion_main!(benches);
