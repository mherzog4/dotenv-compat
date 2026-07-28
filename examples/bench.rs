//! Rough throughput numbers for `parse()`.
//!
//!   cargo run --release --example bench
//!
//! `scripts/bench.mjs` builds byte-identical inputs and times the reference
//! implementation, so the two outputs can be compared directly.

use std::time::Instant;

fn main() {
    for (label, lines) in [("small", 20), ("medium", 400), ("large", 40_000)] {
        let input = synthetic(lines);
        let bytes = input.len();

        // Enough repetitions that even the small case runs long enough to time.
        let reps = (20_000_000 / bytes.max(1)).clamp(3, 20_000);

        let start = Instant::now();
        let mut keys = 0usize;
        for _ in 0..reps {
            keys += std::hint::black_box(dotenv_rs::parse(input.as_bytes())).len();
        }
        let elapsed = start.elapsed();

        let per_op = elapsed.as_secs_f64() / reps as f64;
        println!(
            "rust  {label:<7} {bytes:>9} B  {:>10.1} us/op  {:>7.1} MB/s  ({} keys)",
            per_op * 1e6,
            bytes as f64 / per_op / 1e6,
            keys / reps
        );
    }
}

/// Deterministic `.env` content mixing every syntax the parser handles.
fn synthetic(lines: usize) -> String {
    let mut out = String::new();
    for i in 0..lines {
        match i % 5 {
            0 => out.push_str(&format!("PLAIN_{i}=value{i}\n")),
            1 => out.push_str(&format!("QUOTED_{i}=\"value {i} with spaces\"\n")),
            2 => out.push_str(&format!("SINGLE_{i}='literal \\n stays {i}'\n")),
            3 => out.push_str(&format!(
                "# comment about key {i}\nCOMMENTED_{i}=v{i} # trailing\n"
            )),
            _ => out.push_str(&format!("export MULTI_{i}=\"line one\\nline two {i}\"\n")),
        }
    }
    out
}
