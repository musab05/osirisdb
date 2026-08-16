use criterion::{Criterion, black_box, criterion_group, criterion_main};
use osirisdb::storage::log::log_manager::LogManager;
use osirisdb::storage::log::log_record::{LogRecord, RecordType};
use std::env::temp_dir;
use std::fs;
use std::sync::Arc;
use std::thread;

fn create_dummy_record() -> LogRecord {
    LogRecord {
        lsn: 0,
        prev_lsn: 0,
        txt_id: 1,
        record_type: RecordType::Insert,
        file_id: 1,
        page_id: 1,
        offset: 0,
        length: 10,
        before_image: vec![],
        after_image: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
    }
}

fn bench_append_record(c: &mut Criterion) {
    let log_path = temp_dir().join("bench_append.log");
    let _ = fs::remove_file(&log_path);

    let manager = LogManager::new(&log_path).unwrap();

    c.bench_function("log_manager_append_record", |b| {
        b.iter(|| {
            let mut record = create_dummy_record();
            manager.append_record(black_box(&mut record)).unwrap();
        })
    });

    let _ = fs::remove_file(&log_path);
}

fn bench_concurrent_append(c: &mut Criterion) {
    let log_path = temp_dir().join("bench_concurrent_append.log");
    let _ = fs::remove_file(&log_path);

    let manager = Arc::new(LogManager::new(&log_path).unwrap());

    c.bench_function("log_manager_concurrent_append_4_threads", |b| {
        b.iter(|| {
            let mut handles = Vec::with_capacity(4);
            for _ in 0..4 {
                let mgr = Arc::clone(&manager);
                handles.push(thread::spawn(move || {
                    for _ in 0..25 {
                        let mut record = create_dummy_record();
                        mgr.append_record(&mut record).unwrap();
                    }
                }));
            }
            for handle in handles {
                handle.join().unwrap();
            }
        })
    });

    let _ = fs::remove_file(&log_path);
}

fn bench_group_commit(c: &mut Criterion) {
    let log_path = temp_dir().join("bench_group_commit.log");
    let _ = fs::remove_file(&log_path);

    let manager = Arc::new(LogManager::new(&log_path).unwrap());

    c.bench_function("log_manager_group_commit_wait", |b| {
        b.iter(|| {
            let mut record = create_dummy_record();
            let lsn = manager.append_record(&mut record).unwrap();
            manager.wait_for_flush(lsn.0).unwrap();
        })
    });

    let _ = fs::remove_file(&log_path);
}

fn bench_flush_record(c: &mut Criterion) {
    let log_path = temp_dir().join("bench_flush.log");
    let _ = fs::remove_file(&log_path);

    let manager = LogManager::new(&log_path).unwrap();

    c.bench_function("log_manager_flush", |b| {
        b.iter(|| {
            let mut record = create_dummy_record();
            manager.append_record(&mut record).unwrap();
            manager.flush().unwrap();
        })
    });

    let _ = fs::remove_file(&log_path);
}

criterion_group!(
    benches,
    bench_append_record,
    bench_concurrent_append,
    bench_group_commit,
    bench_flush_record
);
