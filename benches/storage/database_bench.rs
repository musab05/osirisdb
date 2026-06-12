use criterion::{Criterion, criterion_group, criterion_main};
use osirisdb::storage::Storage;
use std::hint::black_box;

fn bench_create_database_dir(c: &mut Criterion) {
    let tmp = std::env::temp_dir().join("osirisdb_bench_storage");
    // clean up any leftover from previous runs before starting
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let storage = Storage::new(&tmp).unwrap();

    let mut counter = 0u64;
    c.bench_function("storage_create_database_dir", |b| {
        b.iter(|| {
            let name = format!("bench_db_{}", counter);
            counter += 1;
            storage.create_database_dir(black_box(&name)).unwrap();
        })
    });

    std::fs::remove_dir_all(&tmp).unwrap();
}

fn bench_drop_database_dir(c: &mut Criterion) {
    let tmp = std::env::temp_dir().join("osirisdb_bench_storage_drop");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let storage = Storage::new(&tmp).unwrap();

    let mut counter = 0u64;
    c.bench_function("storage_drop_database_dir", |b| {
        b.iter_batched(
            || {
                let name = format!("bench_db_{}", counter);
                counter += 1;
                storage.create_database_dir(&name).unwrap();
                name
            },
            |name| storage.drop_database_dir(black_box(&name)).unwrap(),
            criterion::BatchSize::SmallInput,
        )
    });

    std::fs::remove_dir_all(&tmp).unwrap();
}

criterion_group!(benches, bench_create_database_dir, bench_drop_database_dir);
criterion_main!(benches);
