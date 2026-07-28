//! Port of dotenv's `parse()`.
//!
//! The reference implementation drives one global regex over the whole buffer:
//!
//! ```text
//! /(?:^|^)\s*(?:export\s+)?([\w.-]+)(?:\s*=\s*?|:\s+?)(\s*'(?:\\'|[^'])*'|\s*"(?:\\"|[^"])*"|\s*`(?:\\`|[^`])*`|[^#\r\n]+)?\s*(?:#.*)?(?:$|$)/mg
//! ```
//!
//! followed by `trim()`, a quote-stripping `replace()`, and `\n`/`\r` expansion for
//! double-quoted values. This module reproduces those steps directly, including the
//! backtracking behaviour of the quoted alternatives, rather than splitting on lines
//! (a quoted value may span newlines, so line splitting is not equivalent).
//!
//! JavaScript treats U+2028 and U+2029 as line terminators for `^`, `$` and `.`
//! (but they are NOT excluded by the `[^#\r\n]` value class, and they ARE matched
//! by `\s`), so `is_line_term` covers all three. `\r` is normalised away first.

use crate::map::EnvMap;
use std::borrow::Cow;

/// Parse the contents of a `.env` file into a map of key/value pairs.
///
/// Later assignments to the same key win, matching the reference implementation.
pub fn parse(src: &[u8]) -> EnvMap {
    // `Buffer.prototype.toString()` replaces invalid sequences rather than failing.
    let text = String::from_utf8_lossy(src);
    // /\r\n?/mg -> '\n'
    let normalized: Cow<str> = if text.contains('\r') {
        Cow::Owned(text.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        text
    };
    // ponytail: a Vec<char> costs one pass and 4 bytes per character, so peak
    // memory is a multiple of the input (~6x ASCII, ~17x for mostly-invalid UTF-8,
    // where each bad byte becomes a 4-byte replacement char). It keeps the index
    // arithmetic below a direct transcription of the regex. Switch to byte offsets
    // over the &str if that ever matters.
    let chars: Vec<char> = normalized.chars().collect();

    let mut out = EnvMap::new();
    let mut pos = 0usize;

    while pos <= chars.len() {
        // Leading `\s*` is greedy and keys never start with whitespace, so the first
        // non-whitespace character is the only place a match can begin.
        let start = skip_space(&chars, pos);

        match match_entry(&chars, start) {
            Some(entry) => {
                // `obj[key] = value` on a plain object: assigning `__proto__` hits
                // the prototype setter, so a string value creates no own property
                // and the key silently disappears from the result.
                if entry.key != "__proto__" {
                    out.insert(entry.key, finish_value(&entry.raw_value));
                }
                // A match always ends at a `$`, i.e. at a newline or at the end of
                // input. Resume at the next position where `^` can match.
                pos = if entry.end == 0 || is_line_term(chars[entry.end - 1]) {
                    entry.end
                } else {
                    next_line(&chars, entry.end)
                };
            }
            None => {
                if start >= chars.len() {
                    break;
                }
                pos = next_line(&chars, start);
            }
        }
    }

    out
}

struct Entry {
    key: String,
    /// Capture group 2 exactly as the regex saw it, before `trim()`.
    raw_value: String,
    /// Index one past the end of the whole match.
    end: usize,
}

/// `(?:export\s+)?([\w.-]+)(?:\s*=\s*?|:\s+?)(value)?\s*(?:#.*)?$` anchored at `start`.
fn match_entry(c: &[char], start: usize) -> Option<Entry> {
    // `(?:export\s+)?` is greedy: the engine tries with the prefix, then without.
    for use_export in [true, false] {
        let mut p = start;

        if use_export {
            if !starts_with(c, p, "export") {
                continue;
            }
            let after = p + "export".len();
            let w = skip_space(c, after);
            if w == after {
                continue; // `\s+` needs at least one whitespace character
            }
            p = w;
        }

        // `([\w.-]+)`. A shorter key can never help: the character that ends the run
        // is neither whitespace, `=` nor `:`, so no separator could follow it.
        let key_end = p + c[p..].iter().take_while(|ch| is_key_char(**ch)).count();
        if key_end == p {
            continue;
        }
        let key: String = c[p..key_end].iter().collect();

        // `(?:\s*=\s*?|:\s+?)`, in alternation order.
        let mut separators = Vec::new();
        let eq = skip_space(c, key_end);
        if c.get(eq) == Some(&'=') {
            separators.push(eq + 1);
        }
        if c.get(key_end) == Some(&':') && skip_space(c, key_end + 1) > key_end + 1 {
            // `\s+?` is lazy; one whitespace character is always enough, because any
            // remainder is absorbed by the value alternative or by the trailing `\s*`.
            separators.push(key_end + 2);
        }

        for value_start in separators {
            if let Some((raw_value, end)) = match_value(c, value_start) {
                return Some(Entry {
                    key,
                    raw_value,
                    end,
                });
            }
        }
    }

    None
}

/// Match the optional value group plus the trailing `\s*(?:#.*)?$`.
///
/// Returns the raw capture and the end of the whole match.
fn match_value(c: &[char], start: usize) -> Option<(String, usize)> {
    for quote in ['\'', '"', '`'] {
        for alt_end in quoted_candidates(c, start, quote) {
            if let Some(end) = match_tail(c, alt_end) {
                return Some((c[start..alt_end].iter().collect(), end));
            }
        }
    }

    // `[^#\r\n]+`. Backtracking to a shorter run is never needed: the run stops at
    // `#`, at a newline or at the end of input, and the trailing context accepts all
    // three.
    let run_end = start
        + c[start..]
            .iter()
            .take_while(|ch| !matches!(**ch, '#' | '\r' | '\n'))
            .count();
    if run_end > start {
        if let Some(end) = match_tail(c, run_end) {
            return Some((c[start..run_end].iter().collect(), end));
        }
    }

    // The group is optional; an absent group captures `undefined`, defaulted to "".
    match_tail(c, start).map(|end| (String::new(), end))
}

/// End positions for `\s*Q(?:\\Q|[^Q])*Q`, in the order the regex engine would try them.
///
/// The star is greedy and prefers consuming `\Q` as an escape pair, so the first
/// candidate is the closing quote found by a straight greedy scan. Each subsequent
/// candidate comes from undoing one escape pair (most recent first), which lets its
/// quote act as the terminator instead.
fn quoted_candidates(c: &[char], start: usize, quote: char) -> Vec<usize> {
    let open = skip_space(c, start);
    if c.get(open) != Some(&quote) {
        return Vec::new();
    }

    let mut escaped_quotes = Vec::new();
    let mut greedy_close = None;
    let mut i = open + 1;
    while i < c.len() {
        if c[i] == '\\' && c.get(i + 1) == Some(&quote) {
            escaped_quotes.push(i + 1);
            i += 2;
        } else if c[i] != quote {
            i += 1;
        } else {
            greedy_close = Some(i);
            break;
        }
    }

    let mut ends: Vec<usize> = greedy_close.into_iter().map(|q| q + 1).collect();
    ends.extend(escaped_quotes.iter().rev().map(|q| q + 1));
    ends
}

/// Match `\s*(?:#.*)?$` (multiline) at `pos`, returning the end of the match.
fn match_tail(c: &[char], pos: usize) -> Option<usize> {
    // `\s*` is greedy, so the longest whitespace run is tried first.
    let run_end = skip_space(c, pos);

    if run_end == c.len() {
        return Some(run_end);
    }
    if c[run_end] == '#' {
        // `.` does not cross a line terminator, so the comment runs to end of line.
        return Some(next_newline(c, run_end));
    }

    // Otherwise back off to the last `$` inside the whitespace run. `#` is not
    // whitespace, so no comment can start there -- only a newline can end the match.
    c[pos..run_end]
        .iter()
        .rposition(|ch| is_line_term(*ch))
        .map(|offset| pos + offset)
}

/// `trim()`, then quote stripping, then `\n`/`\r` expansion for double-quoted values.
fn finish_value(raw: &str) -> String {
    let trimmed = js_trim(raw);
    // The reference reads the quote character *before* stripping, so an unbalanced
    // leading `"` still triggers escape expansion.
    let maybe_quote = trimmed.chars().next();
    let stripped = strip_quotes(trimmed);

    if maybe_quote == Some('"') {
        // Only `\n` and `\r` are expanded; `\\`, `\"` and `\t` are left alone.
        stripped.replace("\\n", "\n").replace("\\r", "\r")
    } else {
        stripped.into_owned()
    }
}

/// `value.replace(/^(['"`])([\s\S]*)\1$/mg, '$2')`.
fn strip_quotes(s: &str) -> Cow<'_, str> {
    // Without a newline there is exactly one place `^` can match, so the whole
    // replace collapses to "strip a matched pair of outer quotes". This is the
    // shape of virtually every real value, and it needs no allocation.
    if !s.contains(is_line_term) {
        let bytes = s.as_bytes();
        let quote = match bytes.first() {
            Some(&q @ (b'\'' | b'"' | b'`')) => q,
            _ => return Cow::Borrowed(s),
        };
        // The quote characters are ASCII, so these indices are char boundaries.
        return match bytes.len() >= 2 && bytes[bytes.len() - 1] == quote {
            true => Cow::Borrowed(&s[1..s.len() - 1]),
            false => Cow::Borrowed(s),
        };
    }

    Cow::Owned(strip_quotes_multiline(s))
}

/// General form of [`strip_quotes`], for values containing a newline.
fn strip_quotes_multiline(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut copied = 0usize;
    let mut i = 0usize;

    while i < c.len() {
        // `^` under the `m` flag.
        let at_line_start = i == 0 || is_line_term(c[i - 1]);
        if at_line_start {
            if let Some(close) = matching_close(&c, i) {
                out.extend(&c[copied..i]);
                out.extend(&c[i + 1..close]);
                copied = close + 1;
                i = close + 1; // the `g` flag resumes at the end of the match
                continue;
            }
        }
        i += 1;
    }

    out.extend(&c[copied..]);
    out
}

/// Index of the quote closing a `^(['"`])([\s\S]*)\1$` match starting at `start`.
fn matching_close(c: &[char], start: usize) -> Option<usize> {
    let quote = *c.get(start)?;
    if !matches!(quote, '\'' | '"' | '`') {
        return None;
    }
    // `[\s\S]*` is greedy, so prefer the last position that satisfies `\1$`.
    (start + 1..c.len())
        .rev()
        .find(|&e| c[e] == quote && (e + 1 == c.len() || is_line_term(c[e + 1])))
}

fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

/// JavaScript's `\s`: Unicode `White_Space` minus U+0085, plus U+FEFF.
fn is_js_space(ch: char) -> bool {
    (ch.is_whitespace() && ch != '\u{85}') || ch == '\u{feff}'
}

/// The `[\w.-]` character class. JavaScript's `\w` is ASCII-only without the `u` flag.
fn is_key_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-')
}

fn skip_space(c: &[char], from: usize) -> usize {
    from + c[from..].iter().take_while(|ch| is_js_space(**ch)).count()
}

fn next_newline(c: &[char], from: usize) -> usize {
    c[from..]
        .iter()
        .position(|ch| is_line_term(*ch))
        .map_or(c.len(), |offset| from + offset)
}

/// A JavaScript LineTerminator, as `^`, `$` and `.` see it. `\r` never reaches
/// here because normalisation rewrites it to `\n` first.
fn is_line_term(ch: char) -> bool {
    matches!(ch, '\n' | '\u{2028}' | '\u{2029}')
}

fn next_line(c: &[char], from: usize) -> usize {
    match next_newline(c, from) {
        n if n == c.len() => n,
        n => n + 1,
    }
}

fn starts_with(c: &[char], at: usize, word: &str) -> bool {
    let n = word.len();
    at + n <= c.len() && c[at..at + n].iter().copied().eq(word.chars())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `strip_quotes` has a no-newline fast path and a general path. They must
    /// agree wherever both are defined, or a value silently changes shape
    /// depending on whether it happens to contain a line terminator.
    #[test]
    fn strip_quotes_paths_agree() {
        let alphabet = ['\'', '"', '`', 'a', '\\', ' '];
        let mut checked = 0usize;

        for len in 0..=5 {
            let mut indices = vec![0usize; len];
            loop {
                let s: String = indices.iter().map(|&i| alphabet[i]).collect();
                assert_eq!(
                    strip_quotes(&s).into_owned(),
                    strip_quotes_multiline(&s),
                    "paths disagree on {s:?}"
                );
                checked += 1;

                let mut at = len;
                loop {
                    if at == 0 {
                        break;
                    }
                    at -= 1;
                    indices[at] += 1;
                    if indices[at] < alphabet.len() {
                        break;
                    }
                    indices[at] = 0;
                    if at == 0 {
                        break;
                    }
                }
                if len == 0 || indices.iter().all(|&i| i == 0) {
                    break;
                }
            }
        }

        assert!(checked > 9000, "only checked {checked} strings");
    }

    /// `parse` is fed untrusted file content and must never panic, whatever the
    /// bytes are. Exhaustive over short strings from a hostile alphabet.
    #[test]
    fn never_panics_on_short_inputs() {
        let alphabet = [
            'A', '=', '\'', '"', '`', '\\', '#', '\n', ' ', ':', '\u{2028}', '\u{0}',
        ];
        let mut buf = String::new();

        for len in 0..=4 {
            let total = alphabet.len().pow(len as u32);
            for mut n in 0..total {
                buf.clear();
                for _ in 0..len {
                    buf.push(alphabet[n % alphabet.len()]);
                    n /= alphabet.len();
                }
                // The assertion is simply that this returns.
                let _ = parse(buf.as_bytes());
            }
        }
    }

    /// The outer loop must always advance, on any input.
    #[test]
    fn terminates_on_pathological_input() {
        for input in [
            "=".repeat(5000),
            "\u{2028}".repeat(5000),
            "'".repeat(5000),
            "\\'".repeat(5000),
            format!("A='{}", "a\\'".repeat(2000)),
            format!("A=\"{}", "\\\"".repeat(2000)),
            format!("{}A=1", " ".repeat(5000)),
            format!("{}\nA=1", "x".repeat(5000)),
        ] {
            let _ = parse(input.as_bytes());
        }
    }
}
