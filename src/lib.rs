//! A Rust port of the JavaScript [dotenv](https://github.com/motdotla/dotenv) library.
//!
//! Behaviour is a bug-for-bug port of the `lib/main.js` that `dotenv@17.4.2`
//! ships on npm, quirks included: the parser is validated against the reference
//! implementation by recorded vectors and two differential fuzzers (see
//! `scripts/`).
//!
//! ```no_run
//! // Call before spawning threads -- see the safety note on `config`.
//! let result = dotenv_compat::config();
//! println!("loaded {} keys", result.parsed.len());
//! ```
//!
//! To parse without touching the environment:
//!
//! ```
//! let parsed = dotenv_compat::parse(b"HOST=localhost\nPORT=8080");
//! assert_eq!(parsed["PORT"], "8080");
//! ```

mod config;
mod error;
mod map;
mod nodepath;
mod parser;
mod populate;

pub use config::{ConfigResult, Options, config, config_with};
pub use error::Error;
pub use map::EnvMap;
pub use parser::parse;
pub use populate::populate;

/// The readme's examples are compiled and run as doctests, so they cannot rot.
#[cfg(doctest)]
#[doc = include_str!("../readme.md")]
mod readme {}
