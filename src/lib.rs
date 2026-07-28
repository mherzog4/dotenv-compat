//! A Rust port of the JavaScript [dotenv](https://github.com/motdotla/dotenv) library.
//!
//! Behaviour is a faithful port of `dotenv@17.4.2`, quirks included.

use std::collections::HashMap;

/// Parse the contents of a `.env` file into a map of key/value pairs.
///
/// Invalid UTF-8 is replaced lossily, matching `Buffer.prototype.toString()`.
pub fn parse(src: &[u8]) -> HashMap<String, String> {
    let _ = src;
    todo!("phase 2")
}
