//! Batch runner used by the differential fuzzers in `scripts/`.
//!
//! Two modes, selected by argv[1] (default `parse`):
//!
//! `parse` -- exercises `parse()`.
//!   stdin:  repeated [u32 len][input bytes]
//!   stdout: per case, a pair map (see `write_map`)
//!
//! `config` -- exercises `config_with()`, including file cascade and the
//!   already-set-in-the-environment branch.
//!   stdin:  repeated
//!             [u32 path count] then per path [u32 len][utf-8 path]
//!             [u32 preset count] then per preset [u32 len][key][u32 len][value]
//!             [u8 overwrite]
//!   stdout: per case, [u8 had error] then the `parsed` map, then the resulting
//!           values of every key in (parsed union presets) that is set
//!
//! Length-prefixed frames keep NUL bytes, newlines and quotes intact without
//! needing an encoder on either side.
//!
//! The `config` mode mutates the real process environment, so it restores every
//! key it touched after each case and must stay single-threaded. The fuzzer only
//! generates keys under a reserved prefix, so a collision with a real variable
//! (which would make the process environment and the JS `processEnv` object
//! disagree about what is already set) cannot happen.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;

use dotenv_compat::Options;

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "parse".into());

    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();

    let mut reader = Reader {
        bytes: &input,
        at: 0,
    };
    let mut out = Vec::new();

    match mode.as_str() {
        "parse" => {
            while let Some(payload) = reader.field() {
                write_map(&mut out, &dotenv_compat::parse(payload));
            }
        }
        "config" => {
            while reader.at < reader.bytes.len() {
                run_config_case(&mut reader, &mut out);
            }
        }
        other => panic!("unknown mode {other:?}"),
    }

    std::io::stdout().write_all(&out).unwrap();
}

fn run_config_case(reader: &mut Reader, out: &mut Vec<u8>) {
    let paths: Vec<PathBuf> = (0..reader.count())
        .map(|_| PathBuf::from(reader.text()))
        .collect();

    let presets: Vec<(String, String)> = (0..reader.count())
        .map(|_| (reader.text(), reader.text()))
        .collect();

    let overwrite = reader.byte() != 0;

    for (key, value) in &presets {
        // SAFETY: this binary is single-threaded by construction.
        unsafe { std::env::set_var(key, value) };
    }

    let result = dotenv_compat::config_with(&Options {
        path: paths,
        overwrite,
        quiet: true,
        debug: false,
    });

    // Every key either implementation could have touched.
    let mut touched: Vec<String> = result.parsed.keys().cloned().collect();
    touched.extend(presets.iter().map(|(k, _)| k.clone()));
    touched.sort_unstable();
    touched.dedup();

    let resulting: HashMap<String, String> = touched
        .iter()
        .filter_map(|key| std::env::var(key).ok().map(|value| (key.clone(), value)))
        .collect();

    out.push(u8::from(result.error.is_some()));
    write_map(out, &result.parsed);
    write_map(out, &resulting);

    for key in &touched {
        // SAFETY: single-threaded, as above.
        unsafe { std::env::remove_var(key) };
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn field(&mut self) -> Option<&'a [u8]> {
        if self.at + 4 > self.bytes.len() {
            return None;
        }
        let len = self.count() as usize;
        let payload = &self.bytes[self.at..self.at + len];
        self.at += len;
        Some(payload)
    }

    fn count(&mut self) -> u32 {
        let raw = self.bytes[self.at..self.at + 4].try_into().unwrap();
        self.at += 4;
        u32::from_le_bytes(raw)
    }

    fn byte(&mut self) -> u8 {
        let value = self.bytes[self.at];
        self.at += 1;
        value
    }

    fn text(&mut self) -> String {
        String::from_utf8(self.field().unwrap().to_vec()).unwrap()
    }
}

fn write_map(out: &mut Vec<u8>, map: &HashMap<String, String>) {
    out.extend_from_slice(&(map.len() as u32).to_le_bytes());
    for (key, value) in map {
        write_field(out, key.as_bytes());
        write_field(out, value.as_bytes());
    }
}

fn write_field(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}
