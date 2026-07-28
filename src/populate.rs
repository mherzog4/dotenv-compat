//! Port of dotenv's `populate()`.

use std::collections::HashSet;

use crate::config::Options;
use crate::map::EnvMap;

/// Copy `parsed` into `target`, returning only the entries that were actually set.
///
/// Existing keys are left alone unless [`Options::overwrite`] is set. This is the
/// pure counterpart of [`crate::config`], useful for applying a parsed `.env` to
/// something other than the process environment.
pub fn populate(target: &mut EnvMap, parsed: &EnvMap, options: &Options) -> EnvMap {
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
///
/// Note the debug flag here is [`Options::populate_debug`], falling back to
/// [`Options::debug`]: the reference's `populate` applies `Boolean()` to the raw
/// option while `configDotenv` applies `parseBoolean`, so the two functions
/// genuinely disagree about what "debug" means when it came from the environment.
pub(crate) fn populate_with(
    parsed: &EnvMap,
    options: &Options,
    exists: impl Fn(&str) -> bool,
    set: &mut dyn FnMut(&str, &str),
) -> EnvMap {
    let mut populated = EnvMap::new();

    for (key, value) in parsed {
        // `processEnv[key] = ...` and `populated[key] = ...` both hit the prototype
        // setter for `__proto__`, so neither assignment takes effect.
        if key == "__proto__" {
            continue;
        }

        if exists(key) {
            if options.overwrite {
                set(key, value);
                populated.insert(key.clone(), value.clone());
            }
            if options.populate_debug.unwrap_or(options.debug) {
                let verb = if options.overwrite {
                    "WAS overwritten"
                } else {
                    "was NOT overwritten"
                };
                crate::config::debug_log(&format!("\"{key}\" is already defined and {verb}"));
            }
        } else {
            set(key, value);
            populated.insert(key.clone(), value.clone());
        }
    }

    populated
}
