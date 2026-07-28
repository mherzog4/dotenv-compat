# Changelog

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
- `EnvMap` — insertion-ordered map, so results stay in file order like a plain
  JavaScript object.
- `Options`, `ConfigResult`, `Error` (with `kind()` as the `err.code` equivalent).

### Testing

- 127 vectors in `tests/vectors.json`, with expectations recorded from the
  reference implementation rather than hand-written.
- Two differential fuzzers (`scripts/fuzz.mjs` for `parse`, `scripts/fuzz-config.mjs`
  for `config`), run on Linux, macOS and Windows in CI.
- A split-context adversarial review of every function against the JavaScript
  source, which found 20 divergences the fuzzers could not reach — including a
  process-killing panic on a NUL byte and a wrong value for U+2028.
- Readme examples run as doctests.

### Known differences

See "Differences from the JavaScript version" in the readme: no `.env.vault`
decryption, no `encoding` option, NUL truncation instead of a panic, and no
`getpwuid()` fallback when `HOME` is unset.
