# dotenv-compat

A Rust port of the JavaScript [dotenv](https://github.com/motdotla/dotenv) library.

The goal is behavioural equivalence with `dotenv@17.4.2`, quirks included. The parser
is checked against the reference implementation by 107 recorded test vectors and a
differential fuzzer; at the time of writing, 1,040,000 random inputs produce
byte-identical output.

No dependencies.

## Usage

```rust
fn main() {
    // Load ./.env into the process environment. Call before spawning threads.
    dotenv_compat::config();

    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    println!("listening on {port}");
}
```

Loading specific files, or letting `.env` values win over what is already set:

```rust
use dotenv_compat::Options;

let result = dotenv_compat::config_with(&Options {
    path: vec![".env".into(), "~/.env.local".into()],
    overwrite: true,
    quiet: true,
    ..Options::default()
});

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
use std::collections::HashMap;
use dotenv_compat::{Options, parse, populate};

let mut target: HashMap<String, String> = HashMap::new();
target.insert("PORT".into(), "9999".into());

let written = populate(&mut target, &parse(b"PORT=8080\nHOST=x"), &Options::default());

assert_eq!(target["PORT"], "9999");  // already present, left alone
assert_eq!(written.len(), 1);        // only HOST was written
```

## API

| Item | Notes |
| --- | --- |
| `parse(&[u8]) -> HashMap<String, String>` | Never fails. Invalid UTF-8 is replaced lossily, matching `Buffer.toString()`. |
| `config() -> ConfigResult` | Loads `./.env`, with defaults from `DOTENV_CONFIG_*`. |
| `config_with(&Options) -> ConfigResult` | Loads the configured files. |
| `populate(&mut HashMap, &HashMap, &Options) -> HashMap` | Pure; returns only the keys it wrote. |
| `Options` | `path`, `overwrite`, `debug`, `quiet`. |
| `ConfigResult` | `parsed`, plus `error` for the last unreadable file. Missing files are not fatal. |

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

| | |
| --- | --- |
| `encoding` option | Not supported. Files are read as UTF-8; `DOTENV_CONFIG_ENCODING` is ignored. |
| `DOTENV_CONFIG_*` | Applied by `Options::from_env()` (used by `config()`). An `Options` you construct is taken verbatim. |
| `processEnv` option | Use `parse` + `populate` against your own `HashMap`. |
| U+2028 / U+2029 | Not treated as line terminators. |
| Summary path display | Never produces `../` segments; unrelated paths are printed in full. |
| Key order | `parse` returns a `HashMap`, so insertion order is not preserved. Last assignment still wins. |

Faithful on purpose: `quiet` defaults to `false`, so `config()` prints
`◇ injected env (N) from .env` to stderr just like the original. Set `quiet: true`
to silence it.

## Thread safety

`config` and `config_with` call `std::env::set_var`, which is unsound if another
thread reads the environment concurrently. Call them early in `main`, before
spawning threads. `parse` and `populate` touch no global state.

## Development

```sh
cargo test                      # 107 vectors + config/populate tests
cargo clippy --all-targets      # clean
cargo run --release --example bench

cd scripts && npm install
npm run gen                     # re-record expectations from dotenv@17.4.2
node fuzz.mjs 100000 7          # differential fuzz against the reference
node bench.mjs                  # same benchmark, reference implementation
```

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
