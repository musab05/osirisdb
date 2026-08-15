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
    let mut manager = LogManager::new(&log_path).unwrap();

    assert_eq!(manager.get_flushed_lsn(), 0);

    let mut record1 = create_dummy_record();
    let lsn1 = manager.append_record(&mut record1).unwrap();
    assert_eq!(lsn1, 1);

    let mut record2 = create_dummy_record();
    let lsn2 = manager.append_record(&mut record2).unwrap();
    assert_eq!(lsn2, 2);

    // Should not be flushed yet
    assert_eq!(manager.get_flushed_lsn(), 0);

    // Flush manually
    manager.flush().unwrap();

    // Now flushed_lsn should be updated to 2
    assert_eq!(manager.get_flushed_lsn(), 2);

    // Clean up
    let _ = fs::remove_file(log_path);
}
