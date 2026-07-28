//! A Rust port of the JavaScript [dotenv](https://github.com/motdotla/dotenv) library.
//!
//! Behaviour is a faithful port of `dotenv@17.4.2`, quirks included: the parser is
//! validated against the reference implementation by 107 recorded vectors and a
//! differential fuzzer (see `scripts/`).
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
mod parser;
mod populate;

pub use config::{ConfigResult, Options, config, config_with};
pub use error::Error;
pub use parser::parse;
pub use populate::populate;
