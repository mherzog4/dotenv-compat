# Changelog

## 0.3.0 — unreleased

### Added

- **`GetUserProfileDirectoryW` fallback** in `os.homedir()`, for Windows with
  `USERPROFILE` unset. This was the last remaining behavioural gap: the port now
  matches the reference everywhere except where the language forbids it.
  Implemented the way libuv does -- `OpenProcessToken` for a query token, a
  null-buffer call to size the result, then the real call -- and reading up to
  the NUL terminator rather than trusting the returned length, also as libuv does.
- A test covering the fallback on both platforms, by unsetting the home variable
  and asserting a leading `~` still expands.

### Dependencies

- `windows-sys` on Windows targets only.

## 0.2.0 — unreleased

Closes the three remaining divergences, so the port is now exact apart from two
differences the language forces.

### Added

- **`.env.vault` decryption.** Full `DOTENV_KEY` support: URL parsing, key
  rotation across comma-separated keys, AES-256-GCM via `aes-gcm`, and the
  `INVALID_DOTENV_KEY` / `DECRYPTION_FAILED` / `NOT_FOUND_DOTENV_ENVIRONMENT` /
  `MISSING_DATA` errors with the reference's exact message text. `config()` is
  now vault-aware; `config_options` is the same with explicit options.
- **The `encoding` option**, covering every Node `Buffer` encoding: `utf8`,
  `utf16le`/`ucs2`, `latin1`/`binary`, `ascii`, `base64`, `base64url` and `hex`,
  plus `DOTENV_CONFIG_ENCODING`. The decoders are hand-written because Node's are
  more lenient than any conforming implementation -- its base64 skips invalid
  characters, accepts both alphabets and needs no padding.
- **`getpwuid()` fallback** in `os.homedir()`, for an unset `HOME` on Unix. An
  *empty* `HOME` is still used as-is, matching libuv.
- `Error::code()`, returning the JavaScript `err.code`.
- `DOTENV_CONFIG_DOTENV_KEY` in `Options::from_env`.

### Changed

- `Error::path()` returns `Option<&Path>`; not every error is about a file.
- `Error::VaultUnsupported` is gone -- the vault is supported now.

### Dependencies

First dependencies: `aes-gcm`, `url`, and `libc` on Unix. The parser and all
encodings remain dependency-free.

### Verification

Every scenario cross-checked against `dotenv@17.4.2`: 9 vault cases (decryption,
rotation, and all 6 error paths with exact message text), 9 encoding cases, and 3
home-directory cases. The vault test fixture was produced by node's `crypto` and
confirmed to decrypt with the reference's own `decrypt`.

## 0.1.0 — unreleased

Initial port of [dotenv](https://github.com/motdotla/dotenv) `17.4.2` to Rust.

The reference is the `lib/main.js` that ships on npm. An earlier draft was
written against the `master` branch on GitHub, which turned out to be a later
refactor (206 lines, no vault, different logging) rather than the released
423-line file; `config()` was re-ported once a review caught that.

### Added

- `parse` — ports the reference `LINE` regex directly, including its backtracking
  behaviour, so quoted values may span newlines and an unterminated quote falls
  back to the unquoted alternative exactly as the regex engine does. Treats
  U+2028/U+2029 as line terminators, and drops a `__proto__` key, both matching
  JavaScript.
- `config` / `config_with` — cascading file loads, lexical `~` expansion via a
  port of Node's `path.join`, `DOTENV_CONFIG_DEBUG`/`QUIET` precedence and
  re-read after population, the `// tip:` summary suffix on stdout, the
  `DOTENV_KEY` missing-vault warning, and non-fatal reporting of unreadable
  files with Node-style `ENOENT:` messages.
- `populate` — pure form, over any `EnvMap`.
- `EnvMap` — ordered map reproducing JavaScript object enumeration: array-index
  keys first in ascending numeric order, then the rest in insertion order.
- `Options`, `ConfigResult`, `Error` (with `kind()` as the `err.code` equivalent).
  Both `Options` and `Error` are `#[non_exhaustive]`; `Options` has `with_*`
  builders.

### Safety

`config` and `config_with` are `unsafe fn`. They write the process environment,
which is undefined behaviour if any other thread reads it concurrently, and the
crate cannot enforce that. `parse` and `populate` are safe.

### Testing

- 127 vectors in `tests/vectors.json`, with expectations recorded from the
  reference implementation rather than hand-written.
- Two differential fuzzers (`scripts/fuzz.mjs` for `parse`, `scripts/fuzz-config.mjs`
  for `config`), run on Linux, macOS and Windows in CI.
- Two split-context adversarial reviews — one against the JavaScript source, one
  hunting logic bugs — which together found 26 issues the fuzzers could not
  reach: a process-killing panic on a NUL byte, a wrong value for U+2028,
  `__proto__` leaking into the environment, JavaScript's array-index key
  ordering, and a safe function wrapping `set_var`.
- Readme examples run as doctests.

### Known differences

See "Differences from the JavaScript version" in the readme: no `.env.vault`
decryption, no `encoding` option, NUL truncation instead of a panic, and no
`getpwuid()` fallback when `HOME` is unset. `parse` also holds the input as a
`Vec<char>`, so peak memory is a multiple of input size.
