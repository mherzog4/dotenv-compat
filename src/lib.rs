//! A Rust port of the JavaScript [dotenv](https://github.com/motdotla/dotenv) library.
//!
//! Behaviour is a faithful port of `dotenv@17.4.2`, quirks included.

mod parser;

pub use parser::parse;
