use crate::storage::{Clog, ClogStatus, TupleHeader, TupleInfoMask, txn::snapshot::Snapshot};

/// Visibility follows rules
pub fn is_tuple_visible(
    header: &mut TupleHeader,
    snapshot: &Snapshot,
    clog: &Clog,
    current_tx: u64,
) -> bool {
    // Rule 1 - Self - inserted tuples
    if header.xmin == current_tx {
        // Did we also delete it in this same transaction
        if header.xmax == current_tx {
            return false; // Created and deleted in current transaction
        }
        return true; // Created by current transaction and not deleted
    }

    // Rule 2 - Check xmin (Tuple Creation)
    if !is_tx_committed(header.xmin, header, true, clog) {
        return false; // Created transaction aborted or still in progress
    }

    // is xmin visible in our snapshot
    if snapshot.is_active(header.xmin) {
        return false; // Creator was active when sncapshot was taken
    }

    // Rule 3 - Check xmax (Tuple Deletion / Update)
    if header.xmax == 0 {
        return true; // Still active/live, never deleted
    }

    if header.xmax == current_tx {
        return false; // Deleted by current transaction
    }

    // If deleter transaction aborted, treat as live
    if is_tx_aborted(header.xmax, header, false, clog) {
        return true;
    }

    // If deleter transaction is still active / uncommitted tuple is still visible
    if !is_tx_committed(header.xmax, header, false, clog) {
        return true;
    }

    // If xmax was committed was it in flight when our snapshot was taken
    if snapshot.is_active(header.xmax) {
        return true; // Deleter committed AFTER our snapshot was taken
    }

    // Deleter committed before our snapshoth -> row is invisible (deleted)
    false
}

fn is_tx_committed(txn_id: u64, header: &mut TupleHeader, is_xmin: bool, clog: &Clog) -> bool {
    let hint_flag = if is_xmin {
        TupleInfoMask::XMIN_COMMITTED
    } else {
        TupleInfoMask::XMAX_COMMITTED
    };

    if TupleInfoMask::is_set(header.infomask, hint_flag) {
        return true;
    }

    match clog.get_status(txn_id) {
        ClogStatus::Committed => {
            // Set hint bit so future skip CLOG
            header.infomask |= hint_flag;
            true
        }
        _ => false,
    }
}

fn is_tx_aborted(txn_id: u64, header: &mut TupleHeader, is_xmin: bool, clog: &Clog) -> bool {
    let hint_flag = if is_xmin {
        TupleInfoMask::XMIN_ABORTED
    } else {
        TupleInfoMask::XMAX_ABORTED
    };

    if TupleInfoMask::is_set(header.infomask, hint_flag) {
        return true;
    }

    match clog.get_status(txn_id) {
        ClogStatus::Aborted => {
            header.infomask |= hint_flag;
            true
        }
        _ => false,
    }
}
