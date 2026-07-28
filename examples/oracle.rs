//! Batch parser used by `scripts/fuzz.mjs` for differential testing.
//!
//! stdin:  repeated [u32 LE byte length][input bytes]
//! stdout: per input, [u32 LE pair count] then per pair
//!         [u32 LE key length][key][u32 LE value length][value]
//!
//! A length-prefixed frame keeps NUL bytes, newlines and quotes intact without
//! needing an encoder on either side.

use std::io::{Read, Write};

fn main() {
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input).unwrap();

    let mut out = Vec::new();
    let mut cursor = 0usize;
    while cursor + 4 <= input.len() {
        let len = u32::from_le_bytes(input[cursor..cursor + 4].try_into().unwrap()) as usize;
        cursor += 4;
        let payload = &input[cursor..cursor + len];
        cursor += len;

        let parsed = dotenv_compat::parse(payload);
        out.extend_from_slice(&(parsed.len() as u32).to_le_bytes());
        for (key, value) in &parsed {
            write_field(&mut out, key.as_bytes());
            write_field(&mut out, value.as_bytes());
        }
    }

    std::io::stdout().write_all(&out).unwrap();
}

fn write_field(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}
