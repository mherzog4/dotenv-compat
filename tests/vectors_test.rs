//! Runs every vector in `tests/vectors.json` against `parse()`.
//!
//! Expectations live in `tests/support/vectors_generated.rs`, recorded from the
//! reference JavaScript implementation. See `scripts/gen_expected.mjs`.

#[path = "support/vectors_generated.rs"]
mod vectors;

use std::collections::HashMap;
use vectors::VECTORS;

#[test]
fn matches_reference_dotenv() {
    let mut failures = Vec::new();

    for v in VECTORS {
        let expected: HashMap<&str, &str> = v.expected.iter().copied().collect();
        let got = dotenv_rs::parse(v.input.as_bytes());
        let got: HashMap<&str, &str> = got
            .iter()
            .map(|(k, val)| (k.as_str(), val.as_str()))
            .collect();

        if got != expected {
            failures.push(format!(
                "  [{}] {}\n    input:    {:?}\n    expected: {:?}\n    got:      {:?}",
                v.category, v.name, v.input, sorted(&expected), sorted(&got)
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} vectors failed:\n{}",
        failures.len(),
        VECTORS.len(),
        failures.join("\n")
    );
}

#[test]
fn vector_set_is_broad_enough() {
    // The porting plan calls for 50+ vectors spanning every syntax category.
    assert!(VECTORS.len() >= 50, "only {} vectors", VECTORS.len());
}

fn sorted<'a>(m: &HashMap<&'a str, &'a str>) -> Vec<(&'a str, &'a str)> {
    let mut v: Vec<_> = m.iter().map(|(k, val)| (*k, *val)).collect();
    v.sort_unstable();
    v
}
