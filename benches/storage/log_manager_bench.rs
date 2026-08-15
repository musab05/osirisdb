use criterion::{black_box, criterion_group, criterion_main, Criterion};
use osirisdb::storage::log::log_manager::LogManager;
use osirisdb::storage::log::log_record::{LogRecord, RecordType};
use std::env::temp_dir;
use std::fs;

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
    // Clean up any leftover from previous runs before starting
    let _ = fs::remove_file(&log_path);
    
    let mut manager = LogManager::new(&log_path).unwrap();

    c.bench_function("log_manager_append_record", |b| {
        b.iter(|| {
            let mut record = create_dummy_record();
            manager.append_record(black_box(&mut record)).unwrap();
        })
    });

    // Clean up
    let _ = fs::remove_file(&log_path);
}

fn bench_flush_record(c: &mut Criterion) {
    let log_path = temp_dir().join("bench_flush.log");
    let _ = fs::remove_file(&log_path);
    
    let mut manager = LogManager::new(&log_path).unwrap();

    c.bench_function("log_manager_flush", |b| {
        b.iter(|| {
            // Append a record to ensure there's something to flush, 
            // otherwise flush is a no-op
            let mut record = create_dummy_record();
            manager.append_record(&mut record).unwrap();
            
            manager.flush().unwrap();
        })
    });

    let _ = fs::remove_file(&log_path);
}

criterion_group!(benches, bench_append_record, bench_flush_record);
