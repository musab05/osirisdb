use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use sysinfo::System;

pub const DEFAULT_MIN_POOL_BYTES: usize = 64 * 1024 * 1024;
pub const DEFAULT_MAX_TOTAL_BYTES: usize = 4 * 1024 * 1024 * 1024; // global ceiling
const MIN_FRAMES: usize = 64;

/// Computed once per process. This is the total byte budget for ALL
/// buffer pools combined — not per-table, per-index.
static TOTAL_BUDGET: OnceLock<usize> = OnceLock::new();

/// How much of TOTAL_BUDGET has already been handed out to opened pools.
static ALLOCATED: AtomicUsize = AtomicUsize::new(0);

/// Computes the process-wide budget once, from total (not "available")
/// system memory — matching how Postgres sizes shared_buffers against
/// total RAM at startup, not against a moment-in-time free-memory reading.
fn total_budget() -> usize {
    *TOTAL_BUDGET.get_or_init(|| {
        let mut system = System::new();
        system.refresh_memory();
        let total = system.total_memory() as usize;

        if total == 0 {
            return DEFAULT_MIN_POOL_BYTES; // detection failed, safe fallback
        }

        (total / 4)
            .min(DEFAULT_MAX_TOTAL_BYTES)
            .max(DEFAULT_MIN_POOL_BYTES)
    })
}

fn frames_from_budget(bytes: usize, page_size: usize) -> usize {
    (bytes / page_size).max(MIN_FRAMES)
}

/// Determines this pool's capacity by drawing from the shared process-wide
/// budget, not by re-querying system memory per call.
///
/// `override_capacity` bypasses all of this for tests / explicit tuning.
pub fn calculate_capacity(page_size: usize, override_capacity: Option<usize>) -> usize {
    if let Some(n) = override_capacity {
        return n.max(1);
    }

    let budget = total_budget();

    // Give each new pool a slice of whatever's left of the shared budget,
    // rather than each one independently claiming a quarter of the whole.
    // Simple fair-share: 1/8th of the ORIGINAL total per pool, capped by
    // whatever's actually left unallocated.
    let already_used = ALLOCATED.load(Ordering::Relaxed);
    let remaining = budget.saturating_sub(already_used);
    let this_pool_share = (budget / 8)
        .min(remaining)
        .max(DEFAULT_MIN_POOL_BYTES.min(remaining.max(1)));

    ALLOCATED.fetch_add(this_pool_share, Ordering::Relaxed);
    frames_from_budget(this_pool_share, page_size)
}
