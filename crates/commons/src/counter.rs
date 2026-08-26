// SPDX-License-Identifier: Apache-2.0 OR MIT

//! A monotonic `u64` counter that works without 64-bit atomics.
//!
//! `std::sync::atomic::AtomicU64` does not exist on targets that lack
//! 64-bit atomic instructions -- `powerpc-unknown-linux-gnu` among them,
//! where importing it fails with `E0432: unresolved import`. That broke
//! every downstream build for those targets.
//!
//! Each counter in this crate is a monotonically increasing sequence
//! number read through `fetch_add`, so a mutex-backed fallback is
//! behaviourally identical. Only the contention profile differs, and
//! these are incremented once per generated identifier rather than in a
//! hot loop.
//!
//! The two definitions are written out rather than wrapped in an inner
//! module, because a re-export would be either an unreachable `pub` or a
//! redundant `pub(crate)` -- both denied by this crate's lints.

use std::sync::atomic::Ordering;

/// Lock-free counter, used wherever the target has 64-bit atomics.
#[cfg(target_has_atomic = "64")]
#[derive(Debug)]
pub struct Counter(std::sync::atomic::AtomicU64);

#[cfg(target_has_atomic = "64")]
impl Counter {
    /// Creates a counter starting at `value`.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(std::sync::atomic::AtomicU64::new(value))
    }

    /// Adds `value` and returns the previous value.
    pub fn fetch_add(&self, value: u64, order: Ordering) -> u64 {
        self.0.fetch_add(value, order)
    }
}

/// Mutex-backed counter for targets without 64-bit atomics.
#[cfg(not(target_has_atomic = "64"))]
#[derive(Debug)]
pub struct Counter(std::sync::Mutex<u64>);

#[cfg(not(target_has_atomic = "64"))]
impl Counter {
    /// Creates a counter starting at `value`.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(std::sync::Mutex::new(value))
    }

    /// Adds `value` and returns the previous value.
    ///
    /// The ordering argument is accepted for API parity and ignored: the
    /// mutex already provides the necessary synchronisation. A poisoned
    /// lock is recovered from rather than panicking, since a counter has
    /// no invariant a panicking writer could corrupt.
    pub fn fetch_add(&self, value: u64, _order: Ordering) -> u64 {
        let mut guard = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let previous = *guard;
        *guard = previous.wrapping_add(value);
        previous
    }
}

#[cfg(test)]
mod tests {
    use super::Counter;
    use std::sync::atomic::Ordering;

    #[test]
    fn fetch_add_returns_previous_and_advances() {
        static C: Counter = Counter::new(5);
        assert_eq!(C.fetch_add(1, Ordering::SeqCst), 5);
        assert_eq!(C.fetch_add(2, Ordering::SeqCst), 6);
        assert_eq!(C.fetch_add(0, Ordering::SeqCst), 8);
    }

    #[test]
    fn counts_correctly_across_threads() {
        static C: Counter = Counter::new(0);
        let mut handles = Vec::new();
        for _ in 0..8 {
            handles.push(std::thread::spawn(|| {
                for _ in 0..1000 {
                    C.fetch_add(1, Ordering::Relaxed);
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        assert_eq!(C.fetch_add(0, Ordering::SeqCst), 8000);
    }
}
