use std::{
    collections::HashMap,
    sync::{Condvar, Mutex},
};

use crate::storage::{
    StorageError,
    txn::lock::{
        lock_mode::RowLockMode,
        lock_table::{LockHead, LockRequest, LockStatus},
        resource::LockResource,
    },
};

pub struct LockManager {
    lock_table: Mutex<HashMap<LockResource, LockHead>>,
}

impl LockManager {
    pub fn new() -> Self {
        Self {
            lock_table: Mutex::new(HashMap::new()),
        }
    }

    /// Acquires a row-level lock on `resource` for `txn_id` in the given `mode`.
    ///
    /// - If `txn_id` already holds an equal-or-stronger lock → no-op.
    /// - If `txn_id` holds a weaker lock → upgrades it in-place.
    /// - If the lock is compatible with all currently granted locks → grants immediately.
    /// - Otherwise → blocks on the `LockHead`'s `Condvar` until granted.
    pub fn acquire(
        &self,
        txn_id: u64,
        resource: LockResource,
        mode: RowLockMode,
    ) -> Result<(), StorageError> {
        let mut table = self.lock_table.lock().unwrap();

        let lock_head = table.entry(resource).or_insert_with(LockHead::new);

        // Check if this txn already holds a lock on this resource
        if let Some(existing) = lock_head
            .granted
            .iter_mut()
            .find(|req| req.txn_id == txn_id)
        {
            // Already holding an equal-or-stronger lock → no-op
            if existing.mode >= mode {
                return Ok(());
            }
            // Holding a weaker lock → upgrade in place
            existing.mode = mode;
            return Ok(());
        }

        // Fresh lock request
        if lock_head.is_compatible(mode, txn_id) {
            // Compatible with all currently granted locks → grant immediately
            lock_head.granted.push(LockRequest {
                txn_id,
                mode,
                status: LockStatus::Granted,
            });
            return Ok(());
        }

        // Incompatible — must wait
        lock_head.waiting.push(LockRequest {
            txn_id,
            mode,
            status: LockStatus::Waiting,
        });

        // Save a pointer to the condvar so we can wait on it.
        // condvar.wait() atomically releases the mutex guard (unblocking other
        // resources) and sleeps. When woken, the guard is re-acquired automatically.
        let condvar = &lock_head.condvar as *const Condvar;

        loop {
            // SAFETY: `condvar` is valid as long as the LockHead lives in the HashMap,
            // and we never remove a LockHead while there are waiters.
            table = unsafe { &*condvar }.wait(table).unwrap();

            // Re-locate our LockHead after re-acquiring the guard
            let lock_head = table.get_mut(&resource).unwrap();

            // Re-check compatibility now that some locks may have been released
            if lock_head.is_compatible(mode, txn_id) {
                // Move ourselves from waiting → granted
                if let Some(pos) = lock_head
                    .waiting
                    .iter()
                    .position(|req| req.txn_id == txn_id)
                {
                    let mut req = lock_head.waiting.remove(pos);
                    req.status = LockStatus::Granted;
                    lock_head.granted.push(req);
                }
                return Ok(());
            }
            // Still incompatible — go back to sleep
        }
    }

    pub fn release(&self, txn_id: u64, resource: LockResource) -> Result<(), StorageError> {
        let mut state = self.lock_table.lock().unwrap();
        if let Some(lock_head) = state.get_mut(&resource) {
            if let Some(pos) = lock_head
                .granted
                .iter()
                .position(|req| req.txn_id == txn_id)
            {
                lock_head.granted.remove(pos);
                lock_head.condvar.notify_all();
            }
        }
        Ok(())
    }

    pub fn release_all(&self, txn_id: u64) -> Result<(), StorageError> {
        let mut state = self.lock_table.lock().unwrap();

        for lock_head in state.values_mut() {
            if let Some(pos) = lock_head
                .granted
                .iter()
                .position(|req| req.txn_id == txn_id)
            {
                lock_head.granted.remove(pos);
                lock_head.condvar.notify_all();
            }
        }
        Ok(())
    }
}
