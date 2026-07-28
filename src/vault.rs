//! Port of dotenv's `.env.vault` support: `_dotenvKey`, `_vaultPath`,
//! `_instructions`, `decrypt`, `_parseVault` and `_configVault`.
//!
//! A `DOTENV_KEY` looks like
//! `dotenv://:key_1234@dotenvx.com/vault/.env.vault?environment=prod`. The
//! password carries the AES-256 key and the `environment` query parameter selects
//! which `DOTENV_VAULT_<ENV>` entry of the vault file to decrypt. Comma-separated
//! keys are tried in order, for key rotation.
//!
//! Where the reference throws, this returns an `Err`: Rust has no exceptions, so
//! the failure surfaces on [`crate::ConfigResult::error`] instead of unwinding.
//! The `code` on each error matches the JavaScript `err.code`.

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use url::Url;

use crate::config::Options;
use crate::encoding::{base64_decode, hex_decode};
use crate::error::Error;
use crate::map::EnvMap;

/// `_dotenvKey(options)` -- the option first, then the environment, else absent.
pub(crate) fn dotenv_key(options: &Options) -> Option<String> {
    options
        .dotenv_key
        .clone()
        .filter(|k| !k.is_empty())
        .or_else(|| std::env::var("DOTENV_KEY").ok().filter(|k| !k.is_empty()))
}

/// `_vaultPath(options)`.
///
/// With explicit paths the reference keeps the *last* one that exists, appending
/// `.vault` unless it is already a vault path. Otherwise it looks for
/// `./.env.vault`. Returns `None` when the candidate does not exist.
pub(crate) fn vault_path(options: &Options) -> Option<std::path::PathBuf> {
    let candidate = match &options.path {
        Some(paths) if !paths.is_empty() => {
            let mut found = None;
            for path in paths {
                if path.exists() {
                    let text = path.to_string_lossy();
                    found = Some(match text.ends_with(".vault") {
                        true => path.clone(),
                        false => std::path::PathBuf::from(format!("{text}.vault")),
                    });
                }
            }
            found?
        }
        _ => crate::config::cwd().join(".env.vault"),
    };

    candidate.exists().then_some(candidate)
}

/// `_instructions(result, dotenvKey)` -- pull the ciphertext and key out of a
/// parsed vault, given one `DOTENV_KEY`.
fn instructions(parsed: &EnvMap, dotenv_key: &str) -> Result<(String, String), Error> {
    let uri = Url::parse(dotenv_key).map_err(|_| {
        Error::InvalidDotenvKey(
            "Wrong format. Must be in valid uri format like \
             dotenv://:key_1234@dotenvx.com/vault/.env.vault?environment=development"
                .into(),
        )
    })?;

    // `uri.password` is empty rather than absent in JavaScript; both are falsy.
    let key = uri.password().unwrap_or_default();
    if key.is_empty() {
        return Err(Error::InvalidDotenvKey("Missing key part".into()));
    }

    let environment = uri
        .query_pairs()
        .find(|(name, _)| name == "environment")
        .map(|(_, value)| value.into_owned())
        .unwrap_or_default();
    if environment.is_empty() {
        return Err(Error::InvalidDotenvKey("Missing environment part".into()));
    }

    let environment_key = format!("DOTENV_VAULT_{}", environment.to_uppercase());
    match parsed.get(&environment_key) {
        Some(ciphertext) if !ciphertext.is_empty() => Ok((ciphertext.clone(), key.to_string())),
        _ => Err(Error::NotFoundDotenvEnvironment(format!(
            "Cannot locate environment {environment_key} in your .env.vault file."
        ))),
    }
}

/// `decrypt(encrypted, keyStr)` -- AES-256-GCM over `nonce || ciphertext || tag`.
///
/// The key is the last 64 characters of the `DOTENV_KEY` password, hex-decoded.
pub fn decrypt(encrypted: &str, key_str: &str) -> Result<String, Error> {
    // `keyStr.slice(-64)` counts UTF-16 code units; for a hex key these are ASCII.
    let tail: String = {
        let chars: Vec<char> = key_str.chars().collect();
        chars[chars.len().saturating_sub(64)..].iter().collect()
    };
    let key_bytes = hex_decode(&tail);

    let invalid_key = || Error::InvalidDotenvKey("It must be 64 characters long (or more)".into());
    if key_bytes.len() != 32 {
        return Err(invalid_key());
    }

    let buffer = base64_decode(encrypted);
    // subarray(0, 12) / subarray(-16) / subarray(12, -16)
    if buffer.len() < 12 + 16 {
        return Err(Error::DecryptionFailed);
    }
    let (nonce, rest) = buffer.split_at(12);
    let (body, tag) = rest.split_at(rest.len() - 16);

    let Ok(key) = <&Key<Aes256Gcm>>::try_from(key_bytes.as_slice()) else {
        return Err(invalid_key());
    };
    let cipher = Aes256Gcm::new(key);

    // The crate's combined form expects ciphertext||tag, which is what the
    // reference reassembles internally.
    let mut sealed = Vec::with_capacity(body.len() + tag.len());
    sealed.extend_from_slice(body);
    sealed.extend_from_slice(tag);

    let Ok(nonce) = <&Nonce<_>>::try_from(nonce) else {
        return Err(Error::DecryptionFailed);
    };

    match cipher.decrypt(nonce, sealed.as_slice()) {
        // `${buffer}` stringifies as UTF-8, replacing anything invalid.
        Ok(plaintext) => Ok(String::from_utf8_lossy(&plaintext).into_owned()),
        Err(_) => Err(Error::DecryptionFailed),
    }
}

/// `_parseVault(options)` -- load the vault file, then try each comma-separated
/// key in turn, surfacing the last error if none succeed.
///
/// Note the reference loads the vault through `configDotenv`, so the
/// `DOTENV_VAULT_*` entries really are injected into the environment as a side
/// effect, and the usual summary line is printed. That is reproduced.
///
/// # Safety
///
/// Writes the process environment; see [`crate::config`].
pub(crate) unsafe fn parse_vault(
    options: &Options,
    path: std::path::PathBuf,
) -> Result<EnvMap, Error> {
    let mut vault_options = options.clone();
    vault_options.path = Some(vec![path.clone()]);

    // SAFETY: forwarded to our own caller.
    let result = unsafe { crate::config_with(&vault_options) };

    let keys = dotenv_key(options).unwrap_or_default();
    let candidates: Vec<&str> = keys.split(',').collect();
    let mut last_error = None;

    for candidate in &candidates {
        match instructions(&result.parsed, candidate.trim())
            .and_then(|(ciphertext, key)| decrypt(&ciphertext, &key))
        {
            Ok(decrypted) => return Ok(crate::parse(decrypted.as_bytes())),
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error.unwrap_or_else(|| {
        Error::MissingData(format!(
            "Cannot parse {} for an unknown reason",
            path.display()
        ))
    }))
}
