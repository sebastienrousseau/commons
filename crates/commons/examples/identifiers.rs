// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Generating identifiers.
//!
//! ```sh
//! cargo run --example identifiers --features id
//! ```

use commons::id::{
    IdFormat, generate_id, generate_prefixed_id, generate_short_id, generate_timestamp_id,
};

fn main() {
    println!("timestamp : {}", generate_timestamp_id());
    println!("short     : {}", generate_short_id());
    println!("prefixed  : {}", generate_prefixed_id("order"));

    for format in [IdFormat::Timestamp, IdFormat::RandomHex, IdFormat::Short] {
        println!("{format:?} -> {}", generate_id(format));
    }

    // Timestamp identifiers embed a monotonic counter, so two taken in
    // the same millisecond still differ.
    let a = generate_timestamp_id();
    let b = generate_timestamp_id();
    assert_ne!(a, b, "timestamp ids must be unique within a millisecond");
    println!("uniqueness holds within the same millisecond");
}
