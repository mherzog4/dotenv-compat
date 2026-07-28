//! Node's `Buffer` encodings, as accepted by `fs.readFileSync(path, { encoding })`.
//!
//! Three of these are not text decodings at all -- `base64`, `base64url` and `hex`
//! re-encode the bytes into a string of that representation, which the parser then
//! reads as if it were the file. The reference allows it, so it is reproduced.
//!
//! The decoders are deliberately hand-written rather than delegated to a crate:
//! Node's are unusually lenient (its base64 skips invalid characters, accepts both
//! alphabets and needs no padding), and a conforming decoder would reject input
//! the reference happily accepts.
//!
//! Known gap: `utf16le` input containing an unpaired surrogate keeps it in
//! JavaScript, but a Rust `String` cannot hold one, so it becomes U+FFFD.

/// A `Buffer` encoding name. Matching is case-insensitive, like `Buffer.isEncoding`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Utf16Le,
    Latin1,
    Ascii,
    Base64,
    Base64Url,
    Hex,
}

impl Encoding {
    /// `Buffer.isEncoding(name)`, returning the encoding it names.
    ///
    /// Note `utf-16` is *not* an alias in Node, only `utf-16le`.
    pub fn from_name(name: &str) -> Option<Encoding> {
        match name.to_ascii_lowercase().as_str() {
            "utf8" | "utf-8" => Some(Encoding::Utf8),
            "utf16le" | "utf-16le" | "ucs2" | "ucs-2" => Some(Encoding::Utf16Le),
            "latin1" | "binary" => Some(Encoding::Latin1),
            "ascii" => Some(Encoding::Ascii),
            "base64" => Some(Encoding::Base64),
            "base64url" => Some(Encoding::Base64Url),
            "hex" => Some(Encoding::Hex),
            _ => None,
        }
    }

    /// `buffer.toString(encoding)`.
    pub fn decode(self, bytes: &[u8]) -> String {
        match self {
            Encoding::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
            Encoding::Utf16Le => {
                // An odd trailing byte is dropped.
                let units: Vec<u16> = bytes
                    .chunks_exact(2)
                    .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                    .collect();
                String::from_utf16_lossy(&units)
            }
            // Every byte is its own code point.
            Encoding::Latin1 => bytes.iter().map(|&b| b as char).collect(),
            // Node masks the high bit rather than rejecting it: 0xE9 becomes 'i'.
            Encoding::Ascii => bytes.iter().map(|&b| (b & 0x7f) as char).collect(),
            Encoding::Base64 => base64_encode(bytes, BASE64_STANDARD, true),
            Encoding::Base64Url => base64_encode(bytes, BASE64_URL, false),
            Encoding::Hex => bytes.iter().map(|b| format!("{b:02x}")).collect(),
        }
    }
}

const BASE64_STANDARD: &[u8; 64] =
    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const BASE64_URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

fn base64_encode(bytes: &[u8], alphabet: &[u8; 64], pad: bool) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);

    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        let digits = [n >> 18, (n >> 12) & 63, (n >> 6) & 63, n & 63];

        // A 1-byte tail carries 2 digits, a 2-byte tail 3.
        let carried = chunk.len() + 1;
        for &d in &digits[..carried] {
            out.push(alphabet[d as usize] as char);
        }
        if pad {
            for _ in carried..4 {
                out.push('=');
            }
        }
    }

    out
}

/// `Buffer.from(text, 'base64')`.
///
/// Node's decoder ignores anything outside the alphabet -- whitespace, padding,
/// stray punctuation -- accepts the standard and URL alphabets interchangeably,
/// and decodes a trailing partial group rather than erroring.
pub fn base64_decode(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0u32;

    for byte in text.bytes() {
        let Some(value) = base64_value(byte) else {
            continue;
        };
        acc = (acc << 6) | value as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }

    out
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    }
}

/// `Buffer.from(text, 'hex')`.
///
/// Decodes byte pairs and stops at the first pair containing a non-hex digit, so
/// `"41zz"` yields one byte and `"4G"` yields none. An odd trailing digit is dropped.
pub fn hex_decode(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);

    for pair in bytes.chunks_exact(2) {
        match (hex_value(pair[0]), hex_value(pair[1])) {
            (Some(hi), Some(lo)) => out.push(hi << 4 | lo),
            _ => break,
        }
    }

    out
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expectations recorded from node v23.
    const SAMPLE: &[u8] = b"A=caf\xe9\nB=\xff\xfe\n";

    #[test]
    fn decodes_like_node() {
        assert_eq!(
            Encoding::Utf8.decode(SAMPLE),
            "A=caf\u{fffd}\nB=\u{fffd}\u{fffd}\n"
        );
        assert_eq!(Encoding::Latin1.decode(SAMPLE), "A=café\nB=ÿþ\n");
        assert_eq!(Encoding::Ascii.decode(SAMPLE), "A=cafi\nB=\u{7f}~\n");
        assert_eq!(Encoding::Base64.decode(SAMPLE), "QT1jYWbpCkI9//4K");
        assert_eq!(Encoding::Base64Url.decode(SAMPLE), "QT1jYWbpCkI9__4K");
        assert_eq!(Encoding::Hex.decode(SAMPLE), "413d636166e90a423dfffe0a");
        // Odd trailing byte is dropped.
        assert_eq!(Encoding::Utf16Le.decode(&[0x41, 0x00, 0x42]), "A");
    }

    #[test]
    fn base64_padding_matches_node() {
        for (len, standard, url) in [
            (1, "/w==", "_w"),
            (2, "//8=", "__8"),
            (3, "////", "____"),
            (4, "/////w==", "_____w"),
            (5, "//////8=", "______8"),
        ] {
            let bytes = vec![0xffu8; len];
            assert_eq!(Encoding::Base64.decode(&bytes), standard, "len {len}");
            assert_eq!(Encoding::Base64Url.decode(&bytes), url, "len {len}");
        }
    }

    #[test]
    fn base64_decode_is_lenient_like_node() {
        for (input, expected) in [
            ("QUJD", "414243"),
            ("QUJD=", "414243"),
            ("QUJ", "4142"),
            ("QU JD", "414243"),
            ("QU\nJD", "414243"),
            ("QUJD!!", "414243"),
            ("-_8=", "fbff"),
            ("+/8=", "fbff"),
            ("", ""),
            ("=", ""),
            ("QQ", "41"),
        ] {
            let got: String = base64_decode(input)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            assert_eq!(got, expected, "base64 {input:?}");
        }
    }

    #[test]
    fn hex_decode_stops_at_the_first_bad_pair() {
        for (input, expected) in [
            ("4142", "4142"),
            ("414", "41"),
            ("41zz", "41"),
            ("", ""),
            ("4G", ""),
            ("ABCD", "abcd"),
            ("abcd", "abcd"),
        ] {
            let got: String = hex_decode(input)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            assert_eq!(got, expected, "hex {input:?}");
        }
    }

    #[test]
    fn recognises_node_encoding_names() {
        for name in ["UTF8", "utf-8", "UTF-16LE", "utf16le", "UCS2", "ucs-2"] {
            assert!(Encoding::from_name(name).is_some(), "{name}");
        }
        // Node does not accept a bare "utf-16".
        assert!(Encoding::from_name("utf-16").is_none());
        assert!(Encoding::from_name("bogus").is_none());
    }
}
