use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use osirisdb::lexer::Lexer;

const MINIMAL: &str = "CREATE DATABASE mydb;";
const WITH_OWNER: &str = "CREATE DATABASE mydb OWNER alice;";
const FULL: &str = "CREATE DATABASE mydb OWNER alice ENCODING 'UTF8' LOCALE 'en_US' TABLESPACE myspace CONNECTION LIMIT 50;";

pub fn bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("lexer_database");

    for (name, sql) in [
        ("minimal", MINIMAL),
        ("with_owner", WITH_OWNER),
        ("full", FULL),
    ] {
        group.throughput(Throughput::Bytes(sql.len() as u64));
        group.bench_with_input(BenchmarkId::new("lex", name), sql, |b, sql| {
            b.iter(|| {
                Lexer::new(black_box(sql)).for_each(|t| {
                    black_box(t);
                })
            })
        });
    }

    group.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
