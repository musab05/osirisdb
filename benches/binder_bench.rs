mod binder;

use criterion::{Criterion, criterion_group, criterion_main};

fn binder_benches(c: &mut Criterion) {
    binder::database_bench::bench(c);
    binder::schema_bench::bench(c);
    binder::table_bench::bench(c);
}

criterion_group!(benches, binder_benches);
criterion_main!(benches);
