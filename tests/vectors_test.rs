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

        // Order matters: the generator emits `Object.entries`, so the recorded
        // sequence is the reference's own enumeration order.
        let want: Vec<&str> = v.expected.iter().map(|(k, _)| *k).collect();
        let have: Vec<&str> = got.keys().map(String::as_str).collect();

        if got != expected || have != want {
            failures.push(format!(
                "  [{}] {}\n    input:    {:?}\n    expected: {:?} order {:?}\n    got:      {:?} order {:?}",
                v.category, v.name, v.input, expected, want, got, have
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

/// JavaScript hoists array-index keys to the front, in ascending numeric order.
#[test]
fn hoists_array_index_keys() {
    let parsed = dotenv_compat::parse(b"ZED=1\n2=two\nAAA=3\n10=ten\n0=zero");
    let order: Vec<&str> = parsed.keys().map(String::as_str).collect();
    assert_eq!(order, ["0", "2", "10", "ZED", "AAA"]);

    // Only canonical decimals below 2^32-1 count as indices.
    let parsed = dotenv_compat::parse(b"B=1\n01=x\n4294967295=y\n7=z\n00=w");
    let order: Vec<&str> = parsed.keys().map(String::as_str).collect();
    assert_eq!(order, ["7", "B", "01", "4294967295", "00"]);
}

#[test]
fn vector_set_is_broad_enough() {
    // The porting plan calls for 50+ vectors spanning every syntax category.
    assert!(VECTORS.len() >= 50, "only {} vectors", VECTORS.len());
}
