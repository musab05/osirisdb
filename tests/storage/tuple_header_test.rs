use osirisdb::storage::tuple::{RecordId, TUPLE_HEADER_SIZE, TupleHeader, TupleInfoMask};

#[test]
fn test_tuple_header_roundtrip() {
    let original = TupleHeader {
        xmin: 100,
        xmax: 105,
        cid: 2,
        ctid: RecordId {
            page_id: 42,
            slot_id: 7,
        },
        infomask: TupleInfoMask::HAS_NULL | TupleInfoMask::XMIN_COMMITTED,
    };
    let bytes = original.to_bytes();
    assert_eq!(bytes.len(), TUPLE_HEADER_SIZE);
    let decoded = TupleHeader::from_bytes(&bytes).unwrap();
    assert_eq!(original, decoded);
    assert!(!decoded.is_active());
    assert!(decoded.is_deleted());
    assert!(TupleInfoMask::is_set(
        decoded.infomask,
        TupleInfoMask::HAS_NULL
    ));
    assert!(TupleInfoMask::is_set(
        decoded.infomask,
        TupleInfoMask::XMIN_COMMITTED
    ));
}

#[test]
fn test_tuple_header_new_live() {
    let header = TupleHeader::new(
        1,
        0,
        RecordId {
            page_id: 10,
            slot_id: 1,
        },
    );
    assert!(header.is_active());
    assert_eq!(header.xmax, 0);
    assert_eq!(header.infomask, 0);
}

#[test]
fn test_tuple_header_update() {
    let mut header = TupleHeader::new(
        1,
        0,
        RecordId {
            page_id: 1,
            slot_id: 0,
        },
    );
    let new_loc = RecordId {
        page_id: 1,
        slot_id: 1,
    };
    header.mark_updated(2, new_loc);
    assert_eq!(header.xmax, 2);
    assert_eq!(header.ctid, new_loc);
    assert!(TupleInfoMask::is_set(
        header.infomask,
        TupleInfoMask::UPDATED
    ));
}

#[test]
fn test_tuple_header_too_short() {
    let small = [0u8; 10];
    assert!(TupleHeader::from_bytes(&small).is_err());
}
