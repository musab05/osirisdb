mod catalog;

use criterion::{Criterion, criterion_group, criterion_main};

fn catalog_benches(c: &mut Criterion) {
    catalog::database_bench::bench(c);
    catalog::schema_bench::bench(c);
    catalog::table_bench::bench(c);
}

criterion_group!(benches, catalog_benches);
criterion_main!(benches);
