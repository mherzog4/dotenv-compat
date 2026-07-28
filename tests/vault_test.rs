//! Tests for `.env.vault` decryption.
//!
//! The fixture below was produced with node's `crypto.createCipheriv` using a
//! fixed key and nonce, and confirmed to decrypt with `dotenv.decrypt` from
//! `dotenv@17.4.2`, so it is a genuine reference artefact rather than something
//! this crate produced for itself.
//!
//! One test function, because `config()` and `DOTENV_KEY` are process-global.

use std::fs;
use std::path::Path;

use dotenv_compat::{Options, decrypt};

/// AES-256-GCM over `nonce || ciphertext || tag`, base64. Decrypts to
/// `VAULT_A=one\nVAULT_B="two three"\n# comment\nVAULT_C=3\n`.
const BLOB: &str = "AAECAwQFBgcICQoLQFngehQWjIumVQr2Z5QxYzpq5UPZcAzwGVYpvkGHCD3B5NYFj4K0Se5csMC59/KcXDCGBoHK3LZ0vwdhvvt1uAIj9yo=";
const KEY_HEX: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

fn key_uri(environment: &str) -> String {
    format!("dotenv://:key_{KEY_HEX}@dotenvx.com/vault/.env.vault?environment={environment}")
}

#[test]
fn vault() {
    let dir = std::env::temp_dir().join(format!("dotenv-compat-vault-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();

    decrypts_the_reference_fixture();
    rejects_a_short_key();
    rejects_a_wrong_key();
    reports_a_malformed_key();
    loads_a_vault_file(&dir);
    falls_back_when_no_vault_exists(&dir);

    fs::remove_dir_all(&dir).ok();
}

fn decrypts_the_reference_fixture() {
    let plaintext = decrypt(BLOB, &format!("key_{KEY_HEX}")).expect("should decrypt");
    assert_eq!(
        plaintext,
        "VAULT_A=one\nVAULT_B=\"two three\"\n# comment\nVAULT_C=3\n"
    );

    // The reference takes the last 64 characters, so any prefix is ignored.
    assert!(decrypt(BLOB, &format!("anything_at_all_{KEY_HEX}")).is_ok());
}

fn rejects_a_short_key() {
    let error = decrypt(BLOB, "key_abc").expect_err("short key should fail");
    assert_eq!(error.code(), Some("INVALID_DOTENV_KEY"));
    assert_eq!(
        error.to_string(),
        "INVALID_DOTENV_KEY: It must be 64 characters long (or more)"
    );
}

fn rejects_a_wrong_key() {
    let wrong = "f".repeat(64);
    let error = decrypt(BLOB, &format!("key_{wrong}")).expect_err("wrong key should fail");
    assert_eq!(error.code(), Some("DECRYPTION_FAILED"));
    assert_eq!(
        error.to_string(),
        "DECRYPTION_FAILED: Please check your DOTENV_KEY"
    );
}

fn reports_a_malformed_key() {
    // Tampering with the ciphertext must fail authentication, not return garbage.
    let mut tampered = BLOB.to_string();
    tampered.replace_range(20..21, "Z");
    let error = decrypt(&tampered, &format!("key_{KEY_HEX}")).expect_err("tampered blob");
    assert_eq!(error.code(), Some("DECRYPTION_FAILED"));
}

fn loads_a_vault_file(dir: &Path) {
    let vault = dir.join(".env.vault");
    fs::write(&vault, format!("DOTENV_VAULT_PRODUCTION=\"{BLOB}\"\n")).unwrap();

    let options = Options::default()
        .with_path(Some(vec![vault.clone()]))
        .with_dotenv_key(Some(key_uri("production")))
        .with_quiet(true);

    // SAFETY: this test crate is single-threaded by construction.
    let result = unsafe { dotenv_compat::config_options(&options) };

    assert!(result.error.is_none(), "{:?}", result.error);
    assert_eq!(result.parsed["VAULT_A"], "one");
    assert_eq!(result.parsed["VAULT_B"], "two three");
    assert_eq!(result.parsed["VAULT_C"], "3");
    assert_eq!(std::env::var("VAULT_A").unwrap(), "one");

    // An environment with no entry in the vault is reported, not silently empty.
    let missing = Options::default()
        .with_path(Some(vec![vault.clone()]))
        .with_dotenv_key(Some(key_uri("nosuch")))
        .with_quiet(true);
    // SAFETY: as above.
    let result = unsafe { dotenv_compat::config_options(&missing) };
    let error = result
        .error
        .expect("missing environment should be reported");
    assert_eq!(error.code(), Some("NOT_FOUND_DOTENV_ENVIRONMENT"));
    assert_eq!(
        error.to_string(),
        "NOT_FOUND_DOTENV_ENVIRONMENT: Cannot locate environment \
         DOTENV_VAULT_NOSUCH in your .env.vault file."
    );

    // Key rotation: the first key fails, the second succeeds.
    let rotated = format!(
        "dotenv://:key_{}@dotenvx.com/vault/.env.vault?environment=production,{}",
        "e".repeat(64),
        key_uri("production")
    );
    let options = options.with_dotenv_key(Some(rotated));
    // SAFETY: as above.
    let result = unsafe { dotenv_compat::config_options(&options) };
    assert!(result.error.is_none(), "{:?}", result.error);
    assert_eq!(result.parsed["VAULT_A"], "one");
}

fn falls_back_when_no_vault_exists(dir: &Path) {
    let plain = dir.join("plain.env");
    fs::write(&plain, "VAULT_FALLBACK=yes\n").unwrap();

    // DOTENV_KEY set but no `plain.env.vault` beside it: the reference warns and
    // loads the plain file rather than failing.
    let options = Options::default()
        .with_path(Some(vec![plain]))
        .with_dotenv_key(Some(key_uri("production")))
        .with_quiet(true);

    // SAFETY: this test crate is single-threaded by construction.
    let result = unsafe { dotenv_compat::config_options(&options) };

    assert!(result.error.is_none(), "{:?}", result.error);
    assert_eq!(result.parsed["VAULT_FALLBACK"], "yes");
}
