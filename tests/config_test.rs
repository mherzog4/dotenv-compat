//! Tests for `config()` and `populate()`.
//!
//! Everything lives in one test function on purpose: both the process environment
//! and `HOME` are global, so parallel test threads would race on them.

use std::fs;
use std::path::{Path, PathBuf};

use dotenv_compat::{EnvMap, Options, populate};

fn quiet(path: Vec<PathBuf>) -> Options {
    Options::default().with_path(Some(path)).with_quiet(true)
}

/// SAFETY: this test crate is single-threaded by construction (see the module docs).
fn load(options: &Options) -> dotenv_compat::ConfigResult {
    unsafe { dotenv_compat::config_with(options) }
}

fn map(pairs: &[(&str, &str)]) -> EnvMap {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

#[test]
fn config_and_populate() {
    let dir = std::env::temp_dir().join(format!("dotenv-compat-test-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();

    populate_leaves_existing_keys_alone();
    populate_overwrites_when_asked();
    config_loads_a_file(&dir);
    config_respects_existing_env(&dir);
    config_cascades_across_files(&dir);
    config_reports_a_missing_file(&dir);
    config_expands_tilde(&dir);

    fs::remove_dir_all(&dir).ok();
}

fn populate_leaves_existing_keys_alone() {
    let mut target = map(&[("KEEP", "original")]);
    let parsed = map(&[("KEEP", "replacement"), ("NEW", "added")]);

    let written = populate(&mut target, &parsed, &Options::default());

    assert_eq!(target["KEEP"], "original");
    assert_eq!(target["NEW"], "added");
    // Only the keys actually written come back.
    assert_eq!(written, map(&[("NEW", "added")]));
}

fn populate_overwrites_when_asked() {
    let mut target = map(&[("KEEP", "original")]);
    let parsed = map(&[("KEEP", "replacement")]);

    let written = populate(
        &mut target,
        &parsed,
        &Options::default().with_overwrite(true),
    );

    assert_eq!(target["KEEP"], "replacement");
    assert_eq!(written, map(&[("KEEP", "replacement")]));
}

fn config_loads_a_file(dir: &Path) {
    let path = dir.join("basic.env");
    fs::write(&path, "DOTENV_RS_A=one\nDOTENV_RS_B=\"two three\"\n").unwrap();

    let result = load(&quiet(vec![path]));

    assert!(result.error.is_none());
    assert_eq!(result.parsed["DOTENV_RS_A"], "one");
    assert_eq!(std::env::var("DOTENV_RS_A").unwrap(), "one");
    assert_eq!(std::env::var("DOTENV_RS_B").unwrap(), "two three");
}

fn config_respects_existing_env(dir: &Path) {
    let path = dir.join("existing.env");
    fs::write(&path, "DOTENV_RS_PRESET=from_file\n").unwrap();

    // SAFETY: single-threaded test.
    unsafe { std::env::set_var("DOTENV_RS_PRESET", "from_env") };

    load(&quiet(vec![path.clone()]));
    assert_eq!(std::env::var("DOTENV_RS_PRESET").unwrap(), "from_env");

    let result = load(&quiet(vec![path]).with_overwrite(true));
    assert_eq!(std::env::var("DOTENV_RS_PRESET").unwrap(), "from_file");
    // `parsed` reports the file contents regardless of what was applied.
    assert_eq!(result.parsed["DOTENV_RS_PRESET"], "from_file");
}

fn config_cascades_across_files(dir: &Path) {
    let first = dir.join("first.env");
    let second = dir.join("second.env");
    fs::write(&first, "DOTENV_RS_CASCADE=first\n").unwrap();
    fs::write(&second, "DOTENV_RS_CASCADE=second\n").unwrap();

    let earlier = load(&quiet(vec![first.clone(), second.clone()]));
    assert_eq!(earlier.parsed["DOTENV_RS_CASCADE"], "first");

    let later = load(&quiet(vec![first, second]).with_overwrite(true));
    assert_eq!(later.parsed["DOTENV_RS_CASCADE"], "second");
}

fn config_reports_a_missing_file(dir: &Path) {
    let missing = dir.join("does-not-exist.env");

    let result = load(&quiet(vec![missing]));

    assert!(result.parsed.is_empty());
    let error = result.error.expect("missing file should be reported");
    assert!(error.to_string().contains("does-not-exist.env"));
}

fn config_expands_tilde(dir: &Path) {
    // Node's `os.homedir()` reads USERPROFILE on Windows and HOME elsewhere, and
    // so does the crate. Setting the wrong one here made this pass on macOS and
    // fail on Windows.
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    let previous = std::env::var_os(var);
    // SAFETY: single-threaded test.
    unsafe { std::env::set_var(var, dir) };

    fs::write(dir.join("tilde.env"), "DOTENV_RS_TILDE=yes\n").unwrap();
    let result = load(&quiet(vec![PathBuf::from("~/tilde.env")]));

    assert!(result.error.is_none(), "{:?}", result.error);
    assert_eq!(result.parsed["DOTENV_RS_TILDE"], "yes");

    // A `~` path is expanded lexically, so `~/../x` never traverses a symlink.
    fs::write(dir.join("sibling.env"), "DOTENV_RS_SIBLING=yes\n").unwrap();
    let nested = dir.join("nested");
    fs::create_dir_all(&nested).unwrap();
    // SAFETY: single-threaded test.
    unsafe { std::env::set_var(var, &nested) };
    let result = load(&quiet(vec![PathBuf::from("~/../sibling.env")]));
    assert!(result.error.is_none(), "{:?}", result.error);
    assert_eq!(result.parsed["DOTENV_RS_SIBLING"], "yes");

    // SAFETY: single-threaded test.
    match previous {
        Some(home) => unsafe { std::env::set_var(var, home) },
        None => unsafe { std::env::remove_var(var) },
    }
}
