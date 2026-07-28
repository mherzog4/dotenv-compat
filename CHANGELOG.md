# Changelog

## 0.1.0 — unreleased

Initial port of [dotenv](https://github.com/motdotla/dotenv) `17.4.2` to Rust.

### Added

- `parse` — ports the reference `LINE` regex directly, including its backtracking
  behaviour, so quoted values may span newlines and an unterminated quote falls
  back to the unquoted alternative exactly as the regex engine does.
- `config` / `config_with` — cascading file loads, `~` expansion, `DOTENV_CONFIG_*`
  defaults via `Options::from_env`, non-fatal reporting of unreadable files.
- `populate` — pure form, over any `HashMap`.
- `Options`, `ConfigResult`, `Error`.

### Testing

- 107 vectors in `tests/vectors.json`, with expectations recorded from the
  reference implementation rather than hand-written.
- `scripts/fuzz.mjs`, a differential fuzzer. 1,040,000 random inputs agree.
- Benchmarks against the reference on identical input.

### Known differences

See "Differences from the JavaScript version" in the readme. In short: no
`encoding` option, `DOTENV_CONFIG_*` only via `Options::from_env`, U+2028/U+2029
are not line terminators, and `parse` returns an unordered `HashMap`.
