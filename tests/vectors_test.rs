//! Runs every vector in `tests/vectors.json` against `parse()`.
//!
//! Expectations live in `tests/support/vectors_generated.rs`, recorded from the
//! reference JavaScript implementation. See `scripts/gen_expected.mjs`.

#[path = "support/vectors_generated.rs"]
mod vectors;

use dotenv_compat::EnvMap;
use vectors::VECTORS;

#[test]
fn matches_reference_dotenv() {
    let mut failures = Vec::new();

    for v in VECTORS {
        let expected: EnvMap = v
            .expected
            .iter()
            .map(|(k, val)| (k.to_string(), val.to_string()))
            .collect();
        let got = dotenv_compat::parse(v.input.as_bytes());

        if got != expected {
            failures.push(format!(
                "  [{}] {}\n    input:    {:?}\n    expected: {:?}\n    got:      {:?}",
                v.category, v.name, v.input, expected, got
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

/// The reference builds a plain object, so results are in file order.
#[test]
fn preserves_file_order() {
    let parsed = dotenv_compat::parse(b"Z=1\nM=2\nA=3\nM=4");
    let order: Vec<&str> = parsed.keys().map(String::as_str).collect();
    // A repeated key keeps its original position and takes the later value.
    assert_eq!(order, ["Z", "M", "A"]);
    assert_eq!(parsed["M"], "4");
}

#[test]
fn vector_set_is_broad_enough() {
    // The porting plan calls for 50+ vectors spanning every syntax category.
    assert!(VECTORS.len() >= 50, "only {} vectors", VECTORS.len());
}
