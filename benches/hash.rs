use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

use max_image_sender::{solve_pow, solve_pow_parallel};

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("solve_pow", |b| {
        b.iter(|| solve_pow(&[13u8; 16], black_box(10)))
    });

    c.bench_function("solve_pow_parallel", |b| {
        b.iter(|| solve_pow_parallel(&[13u8; 16], black_box(10)))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
