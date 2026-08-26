// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Backoff schedules.
//!
//! ```sh
//! cargo run --example retry --features retry
//! ```

use commons::retry::{BackoffStrategy, RetryConfig};
use std::time::Duration;

fn main() {
    // A policy describes *when* to retry; it does not perform the work.
    let policy = RetryConfig::new()
        .max_attempts(5)
        .backoff(BackoffStrategy::Exponential {
            initial: Duration::from_millis(100),
            max: Duration::from_secs(5),
            multiplier: 2.0,
        })
        .jitter(false);

    println!("schedule for {} attempts:", policy.max_attempts);
    for attempt in 0..policy.max_attempts {
        let delay = policy.backoff.delay_for_attempt(attempt);
        println!("  attempt {attempt}: wait {:>6} ms", delay.as_millis());
    }

    // `no_retry` is the explicit way to opt out, clearer at a call site
    // than `max_attempts(1)`.
    let once = RetryConfig::no_retry();
    println!("no_retry max_attempts = {}", once.max_attempts);
}
