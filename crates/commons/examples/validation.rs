// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Validating untrusted input.
//!
//! ```sh
//! cargo run --example validation --features validation
//! ```

use commons::validation::{
    is_valid_email, is_valid_ipv4, is_valid_url, validate_length, validate_not_empty,
};

fn main() {
    // The `validate_*` family returns the input on success, so calls
    // chain without re-borrowing.
    match validate_not_empty("hello") {
        Ok(value) => println!("not empty : {value:?}"),
        Err(e) => println!("rejected  : {e}"),
    }

    match validate_length("abc", 5, 10) {
        Ok(value) => println!("length ok : {value:?}"),
        Err(e) => println!("rejected  : {e}"),
    }

    // The `is_valid_*` family answers a yes/no question.
    for candidate in ["user@example.com", "not-an-email"] {
        println!("{candidate:<20} email? {}", is_valid_email(candidate));
    }
    for candidate in ["https://example.com", "example.com"] {
        println!("{candidate:<20} url?   {}", is_valid_url(candidate));
    }
    for candidate in ["192.168.0.1", "999.1.1.1"] {
        println!("{candidate:<20} ipv4?  {}", is_valid_ipv4(candidate));
    }
}
