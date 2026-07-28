# dotenv-compat

A Rust port of the JavaScript [dotenv](https://github.com/motdotla/dotenv) library.

The goal is behavioural equivalence with the `lib/main.js` that **`dotenv@17.4.2`
ships on npm**, quirks included. (Note the `master` branch on GitHub is a later
refactor with the vault code removed and different logging — that is *not* what
this crate targets.)

Checked against the reference by 127 recorded test vectors and two differential
fuzzers, plus a split-context adversarial review that compared every function
against the JavaScript source line by line.

Four dependencies, each earning its place: `aes-gcm` for `.env.vault`
decryption, `url` for `DOTENV_KEY` parsing (the same WHATWG standard `new URL()`
implements), and `libc` / `windows-sys` for the home-directory fallback when
`HOME` or `USERPROFILE` is unset. Only two are ever compiled for a given target.
The parser itself and all of Node's `Buffer` encodings are dependency-free.

## Usage

```rust,no_run
fn main() {
    // SAFETY: `config` writes the process environment, which is not thread-safe.
    // Call it before spawning any threads.
    unsafe { dotenv_compat::config() };

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    println!("listening on {port}");
}
```

Loading specific files, or letting `.env` values win over what is already set:

```rust,no_run
use dotenv_compat::Options;

let options = Options::default()
    .with_path(Some(vec![".env".into(), "~/.env.local".into()]))
    .with_overwrite(true)
    .with_quiet(true);

// SAFETY: no other thread may touch the environment during this call.
let result = unsafe { dotenv_compat::config_with(&options) };

if let Some(error) = &result.error {
    eprintln!("could not load a .env file: {error}");
}
```

Parsing without touching the environment:

```rust
let parsed = dotenv_compat::parse(b"HOST=localhost\nPORT=8080");
assert_eq!(parsed["PORT"], "8080");
```

Applying a parsed map to something other than the process environment:

```rust
use dotenv_compat::{EnvMap, Options, parse, populate};

let mut target = EnvMap::new();
target.insert("PORT".into(), "9999".into());

let written = populate(&mut target, &parse(b"PORT=8080\nHOST=x"), &Options::default());

assert_eq!(target["PORT"], "9999");  // already present, left alone
assert_eq!(written.len(), 1);        // only HOST was written
```

## API

| Item | Maps to | Notes |
| --- | --- | --- |
| `parse(&[u8]) -> EnvMap` | `parse` | Never fails. Invalid UTF-8 is replaced lossily, matching `Buffer.toString()`. |
| `unsafe fn config() -> ConfigResult` | `config` | Loads `./.env`. Honours `DOTENV_KEY` (see below). |
| `unsafe fn config_with(&Options)` | `configDotenv` | Loads the configured files. No `DOTENV_KEY` handling, same as the reference. |
| `populate(&mut EnvMap, &EnvMap, &Options) -> EnvMap` | `populate` | Pure; returns only the keys it wrote. |
| `Options` | | `#[non_exhaustive]`; build with `Options::default()` and the `with_*` methods. |
| `Options::from_env()` | `lib/env-options.js` | `DOTENV_CONFIG_*` variables. |
| `Options::from_cli(args)` | `lib/cli-options.js` | `dotenv_config_<name>=<value>` arguments. |
| `Options::for_preload(args)` | `dotenv/config` | Both, merged as the preload does. |
| `ConfigResult` | | `parsed`, plus `error` for the last unreadable file. Missing files are not fatal. |
| `unsafe fn config_options(&Options)` | `config(options)` | Vault-aware, with explicit options. |
| `config_into(&mut EnvMap, &Options)` | `config({ processEnv })` | Writes into your map. **Safe** -- touches nothing global. |
| `config_with_into(&mut EnvMap, &Options)` | `configDotenv({ processEnv })` | Same, without `DOTENV_KEY` handling. |
| `decrypt(&str, &str)` | `decrypt` | AES-256-GCM `.env.vault` decryption. |
| `EnvMap` | a JS object | Ordered string map. Array-index keys enumerate first (ascending), then the rest in insertion order -- exactly as a JS object does. |
| `Error::code()` | `err.code` | `"ENOENT"`, `"INVALID_DOTENV_KEY"`, `"DECRYPTION_FAILED"`, … |

`Options::path` is `Option<Vec<PathBuf>>`, mirroring the JavaScript distinction:
`None` loads the default `./.env`, while `Some(vec![])` loads nothing at all.

## Syntax

```sh
BASIC=basic                          # BASIC="basic"
EMPTY=                               # EMPTY=""
  INDENTED=ok                        # leading whitespace is ignored
export EXPORTED=ok                   # the export keyword is stripped
COLON: works                         # a colon separator needs trailing whitespace

QUOTED="  spaces preserved  "
SINGLE='no escape processing'
BACKTICK=`also literal`
MULTILINE="first
second"                              # quoted values may span newlines

VALUE=unquoted # this is a comment
HASH="not # a comment"
URL=http://x.test/p#frag             # -> "http://x.test/p"; # always ends an
                                     #    unquoted value, even without a space
```

Escape handling matches the reference exactly, which is narrower than people expect:

* Double quotes expand **only** `\n` and `\r`. `\\`, `\"` and `\t` are left as-is,
  so `KEY="C:\new\dir"` yields `C:` + newline + `ew\dir`.
* Single quotes and backticks expand nothing.
* An unbalanced leading `"` still triggers `\n` expansion, because the reference
  reads the quote character before stripping quotes.

## Differences from the JavaScript version

The port aims to be exact, including behaviour that is arguably a bug upstream.
Only two differences remain, both forced by the language rather than chosen:

| | |
| --- | --- |
| `path` as a `URL` | The reference accepts a `URL` object because `fs.readFileSync` does. Rust has no such coercion; convert with `url::Url::to_file_path()` and pass the `PathBuf`. Behaviour is identical, only the accepted type differs. |
| `OBJECT_REQUIRED` | `populate` throws this when `parsed` is not an object. Rust's type system makes that unrepresentable, so the error cannot occur. |
| Errors are returned, not thrown | The reference `throw`s for vault failures. Rust has no exceptions, so those surface on `ConfigResult::error` with an empty `parsed`. `Error::code()` gives the JavaScript `err.code`. |
| Unpaired surrogates | With `encoding: "utf16le"`, a lone surrogate survives in JavaScript but cannot exist in a Rust `String`, so it becomes U+FFFD. |

Faithful on purpose, and easy to mistake for bugs:

* `quiet` defaults to `false`, so `config()` prints `◇ injected env (N) from .env // tip: …` to **stdout**.
* `DOTENV_CONFIG_OVERRIDE=false` turns overriding **on**. The reference copies the raw string into `options.override` and applies `Boolean()`, not `parseBoolean`, so every non-empty value is truthy.
* `DOTENV_CONFIG_DEBUG` / `DOTENV_CONFIG_QUIET` are read on every `config_with` call and beat the explicit option. They are re-read after population, so a `.env` that sets `DOTENV_CONFIG_QUIET=true` silences its own summary line.
* A `__proto__` key is silently dropped, because assigning it on a plain JavaScript object hits the prototype setter.
* `populate`'s debug is a *different flag* from `config`'s (`Options::populate_debug`), and `DOTENV_CONFIG_DEBUG=false` silences one while **enabling** the other.
* Integer-like keys enumerate first: `ZED=1 / 2=two / AAA=3` yields `2, ZED, AAA`.
* `encoding: "base64"`, `"hex"` and `"base64url"` do not *decode* the file -- they re-encode its bytes into that representation, which the parser then reads. Usually nonsense, but the reference allows it.
* Loading a vault also injects the `DOTENV_VAULT_*` entries themselves, because the reference reads the vault through `configDotenv`.

## Encrypted `.env.vault`

Set `DOTENV_KEY` (or `Options::with_dotenv_key`) and `config()` decrypts
`.env.vault` instead of reading `.env`. Comma-separated keys are tried in order,
for rotation. With no vault present it warns and falls back to the plain file,
exactly as the reference does.

```rust,no_run
// SAFETY: call before spawning threads.
let result = unsafe { dotenv_compat::config() };
if let Some(error) = &result.error {
    if error.code() == Some("DECRYPTION_FAILED") {
        eprintln!("check your DOTENV_KEY");
    }
}
```

## Thread safety

`config` and `config_with` are `unsafe fn`. They call `std::env::set_var`, which
is undefined behaviour if any other thread reads or writes the environment
concurrently -- including from C code, and including reads this crate never sees.
There is no way for the crate to enforce that, so the obligation is the caller's
and the signature says so. In practice: call them early in `main`, before
spawning threads.

`parse`, `populate`, `config_into` and `config_with_into` are safe and touch no
global state. If you would rather not take on the obligation, use `config_into`
with your own map:

```rust,no_run
use dotenv_compat::{EnvMap, Options};

let mut env = EnvMap::new();
let result = dotenv_compat::config_into(&mut env, &Options::default());
// The process environment is untouched.
```

### Memory

`parse` decodes to a `Vec<char>` to keep the index arithmetic a direct
transcription of the reference regex, so peak memory is a multiple of input size
-- roughly 6x for ASCII and up to 17x for input that is mostly invalid UTF-8
(each bad byte becomes a 4-byte `char`). Fine for `.env` files; do not point it
at a 100 MB blob.

## Development

```sh
cargo test                      # 127 vectors + config/populate tests
cargo clippy --all-targets      # clean
cargo run --release --example bench

cd scripts && npm ci
npm run gen                     # re-record expectations from dotenv@17.4.2

cargo build --release --example oracle
node fuzz.mjs 100000 7          # differential fuzz of parse()
node fuzz-config.mjs 5000 7     # differential fuzz of config(): cascade,
                                #   override, preset keys, missing files
node bench.mjs                  # same benchmark, reference implementation
```

CI runs the full suite plus both differential fuzzers on Linux, macOS and
Windows, and fails if regenerating the recorded vectors produces any diff.

`tests/support/vectors_generated.rs` is generated from `tests/vectors.json` by
running each input through the real dotenv package. Expectations are never
hand-written, so the suite records what dotenv does rather than what we assume.

### Benchmarks

`cargo run --release --example bench` versus `node scripts/bench.mjs`, on the same
input, Apple Silicon, node v23.10.0:

| input | dotenv-compat | dotenv@17.4.2 |
| --- | --- | --- |
| 670 B | 167 MB/s | 117 MB/s |
| 14 KB | 186 MB/s | 114 MB/s |
| 1.6 MB | 175 MB/s | 84 MB/s |

## License

MIT
