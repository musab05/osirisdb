mod executor;

use criterion::{Criterion, criterion_group, criterion_main};

fn executor_benches(c: &mut Criterion) {
    executor::database_bench::bench(c);
    executor::schema_bench::bench(c);
    executor::table_bench::bench(c);
    executor::insert_bench::bench(c);
}

criterion_group!(benches, executor_benches);
criterion_main!(benches);
