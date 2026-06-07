use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rust_sql::parser::parser::Parser;

const MINIMAL: &str = "CREATE DATABASE mydb;";
const WITH_OWNER: &str = "CREATE DATABASE mydb OWNER alice;";
const FULL: &str = "CREATE DATABASE mydb OWNER alice ENCODING 'UTF8' LOCALE 'en_US' TABLESPACE myspace CONNECTION LIMIT 50;";

pub fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("parser_database");

    for (name, sql) in [("minimal", MINIMAL), ("with_owner", WITH_OWNER), ("full", FULL)] {
        group.throughput(Throughput::Bytes(sql.len() as u64));
        group.bench_with_input(BenchmarkId::new("parse", name), sql, |b, sql| {
            b.iter(|| Parser::new(black_box(sql)).parse())
        });
    }

    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);