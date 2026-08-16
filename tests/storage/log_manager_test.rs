use osirisdb::storage::log::log_manager::{LogManager, Lsn};
use osirisdb::storage::log::log_record::{LogRecord, RecordType};
use std::collections::HashSet;
use std::env::temp_dir;
use std::fs;
use std::sync::{Arc, Mutex};
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

#[test]
fn test_log_manager_initialization() {
    let log_path = temp_dir().join("test_init.log");
    let manager = LogManager::new(&log_path).unwrap();

    assert_eq!(manager.get_flushed_lsn(), 0);

    // Clean up
    let _ = fs::remove_file(log_path);
}

#[test]
fn test_log_manager_append_and_flush() {
    let log_path = temp_dir().join("test_append_flush.log");
    let manager = LogManager::new(&log_path).unwrap();

    assert_eq!(manager.get_flushed_lsn(), 0);

    let mut record1 = create_dummy_record();
    let lsn1 = manager.append_record(&mut record1).unwrap();
    assert_eq!(lsn1, Lsn(1));

    let mut record2 = create_dummy_record();
    let lsn2 = manager.append_record(&mut record2).unwrap();
    assert_eq!(lsn2, Lsn(2));

    // Flush manually
    manager.flush().unwrap();

    // Now flushed_lsn should be updated to 2
    assert_eq!(manager.get_flushed_lsn(), 2);

    // Clean up
    let _ = fs::remove_file(log_path);
}

#[test]
fn test_log_manager_group_commit_wait_for_flush() {
    let log_path = temp_dir().join("test_group_commit.log");
    let manager = Arc::new(LogManager::new(&log_path).unwrap());

    let mut record = create_dummy_record();
    let lsn = manager.append_record(&mut record).unwrap();

    // Spawn a worker thread that waits for the background thread to flush
    let manager_clone = Arc::clone(&manager);
    let handle = thread::spawn(move || {
        manager_clone.wait_for_flush(lsn.0).unwrap();
    });

    handle.join().unwrap();
    assert!(manager.get_flushed_lsn() >= lsn.0);

    // Clean up
    let _ = fs::remove_file(log_path);
}

#[test]
fn test_log_manager_concurrent_appenders() {
    let log_path = temp_dir().join("test_concurrent_append.log");
    let manager = Arc::new(LogManager::new(&log_path).unwrap());
    let collected_lsns = Arc::new(Mutex::new(Vec::new()));

    let num_threads = 8;
    let records_per_thread = 100;
    let mut handles = Vec::new();

    for thread_idx in 0..num_threads {
        let mgr = Arc::clone(&manager);
        let lsns_ref = Arc::clone(&collected_lsns);

        handles.push(thread::spawn(move || {
            for i in 0..records_per_thread {
                let mut record = create_dummy_record();
                record.txt_id = (thread_idx * 1000 + i) as u64;
                let lsn = mgr.append_record(&mut record).unwrap();
                lsns_ref.lock().unwrap().push(lsn.0);
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let all_lsns = collected_lsns.lock().unwrap();
    assert_eq!(all_lsns.len(), num_threads * records_per_thread);

    // Verify all LSNs are unique and strictly in range [1..=800]
    let unique_set: HashSet<u64> = all_lsns.iter().copied().collect();
    assert_eq!(unique_set.len(), num_threads * records_per_thread);

    for expected in 1..=(num_threads * records_per_thread) as u64 {
        assert!(unique_set.contains(&expected));
    }

    // Clean up
    let _ = fs::remove_file(log_path);
}

#[test]
fn test_log_manager_concurrent_group_commit() {
    let log_path = temp_dir().join("test_concurrent_group_commit.log");
    let manager = Arc::new(LogManager::new(&log_path).unwrap());

    let num_threads = 10;
    let mut handles = Vec::new();

    for _ in 0..num_threads {
        let mgr = Arc::clone(&manager);
        handles.push(thread::spawn(move || {
            let mut record = create_dummy_record();
            let lsn = mgr.append_record(&mut record).unwrap();
            // Block until this record is safely flushed to disk
            mgr.wait_for_flush(lsn.0).unwrap();
            assert!(mgr.get_flushed_lsn() >= lsn.0);
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert!(manager.get_flushed_lsn() >= num_threads as u64);

    // Clean up
    let _ = fs::remove_file(log_path);
}

#[test]
fn test_log_manager_auto_flush_on_capacity() {
    let log_path = temp_dir().join("test_auto_flush_capacity.log");
    let manager = LogManager::new(&log_path).unwrap();

    // Append enough records to exceed the 4096-byte default capacity
    for _ in 0..100 {
        let mut record = create_dummy_record();
        manager.append_record(&mut record).unwrap();
    }

    // Auto-flush must have occurred because 100 * ~55 bytes > 4096 bytes
    assert!(manager.get_flushed_lsn() > 0);

    // Clean up
    let _ = fs::remove_file(log_path);
}
