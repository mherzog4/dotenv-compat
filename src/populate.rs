//! Port of dotenv's `populate()`.

use std::collections::{HashMap, HashSet};

use crate::config::Options;

/// Copy `parsed` into `target`, returning only the entries that were actually set.
///
/// Existing keys are left alone unless [`Options::overwrite`] is set. This is the
/// pure counterpart of [`crate::config`], useful for applying a parsed `.env` to
/// something other than the process environment.
pub fn populate(
    target: &mut HashMap<String, String>,
    parsed: &HashMap<String, String>,
    options: &Options,
) -> HashMap<String, String> {
    // Snapshotted because `set` needs a mutable borrow of `target` at the same time.
    let existing: HashSet<String> = target.keys().cloned().collect();
    let mut set = |key: &str, value: &str| {
        target.insert(key.to_string(), value.to_string());
    };
    populate_with(parsed, options, |key| existing.contains(key), &mut set)
}

/// Shared body of `populate`, parameterised over how the target is inspected and written.
///
/// `config()` uses this against the real process environment, where existence has to
/// be probed with `var_os` (a variable can hold non-UTF-8 bytes and still exist).
pub(crate) fn populate_with(
    parsed: &HashMap<String, String>,
    options: &Options,
    exists: impl Fn(&str) -> bool,
    set: &mut dyn FnMut(&str, &str),
) -> HashMap<String, String> {
    let mut populated = HashMap::new();

    for (key, value) in parsed {
        if exists(key) {
            if options.overwrite {
                set(key, value);
                populated.insert(key.clone(), value.clone());
            }
            if options.debug {
                let verb = if options.overwrite {
                    "WAS overwritten"
                } else {
                    "was NOT overwritten"
                };
                crate::config::debug(&format!("\"{key}\" is already defined and {verb}"));
            }
        } else {
            set(key, value);
            populated.insert(key.clone(), value.clone());
        }
    }

    populated
}
