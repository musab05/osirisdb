use std::{
    collections::HashMap,
    sync::{Condvar, Mutex},
};

use crate::storage::{
    StorageError, Transaction,
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

    /// Acquires a row-level or table-level lock on `resource` for the given `txn` in `mode`.
    ///
    /// - If `txn` already holds an equal-or-stronger lock → no-op (no duplicate added to `held_locks`).
    /// - If `txn` holds a weaker lock → upgrades it in-place.
    /// - If the lock is compatible with all currently granted locks → grants immediately and pushes `resource` to `txn.held_locks`.
    /// - Otherwise → blocks on the `LockHead`'s `Condvar` until granted, then pushes `resource` to `txn.held_locks`.
    pub fn acquire(
        &self,
        txn: &mut Transaction,
        resource: LockResource,
        mode: RowLockMode,
    ) -> Result<(), StorageError> {
        let txn_id = txn.txn_id;
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
            txn.held_locks.push(resource);
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
                    txn.held_locks.push(resource);
                }
                return Ok(());
            }
            // Still incompatible — go back to sleep
        }
    }

    /// Releases a single lock on `resource` held by `txn_id` and wakes any waiting transactions.
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

    /// Releases all locks held by the transaction in $O(|\text{held\_locks}|)$ time.
    ///
    /// Drains `txn.held_locks` and performs direct key lookups in the lock table,
    /// notifying waiting transactions on each affected resource's `Condvar`.
    pub fn release_all(&self, txn: &mut Transaction) -> Result<(), StorageError> {
        let mut state = self.lock_table.lock().unwrap();

        for resource in txn.held_locks.drain(..) {
            if let Some(lock_head) = state.get_mut(&resource) {
                if let Some(pos) = lock_head
                    .granted
                    .iter()
                    .position(|req| req.txn_id == txn.txn_id)
                {
                    lock_head.granted.remove(pos);
                    lock_head.condvar.notify_all();
                }
            }
        }
        Ok(())
    }
}
