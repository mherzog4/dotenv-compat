//! The slices of Node's `path` module that dotenv depends on.
//!
//! `_resolveHome` uses `path.join`, and the summary line uses `path.relative`.
//! Both are *lexical* on the JavaScript side -- they never touch the filesystem,
//! so `~/../x` resolves against the textual parent, not through a symlink.
//! `PathBuf::join` and `Path::strip_prefix` do neither of those things, so the
//! behaviour is reproduced here.
//!
//! Node picks `path.posix` or `path.win32` by platform; so do we.

const WINDOWS: bool = cfg!(windows);

fn is_sep(ch: char) -> bool {
    ch == '/' || (WINDOWS && ch == '\\')
}

fn has_drive(path: &str) -> bool {
    let b = path.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

fn sep() -> char {
    if WINDOWS { '\\' } else { '/' }
}

/// `path.join(a, b)` -- concatenate, then normalise `.` and `..` lexically.
pub fn join(base: &str, rest: &str) -> String {
    let joined = match (base.is_empty(), rest.is_empty()) {
        (true, true) => return ".".into(),
        (false, true) => base.to_string(),
        (true, false) => rest.to_string(),
        (false, false) => format!("{base}{}{rest}", sep()),
    };
    normalize(&joined)
}

/// `path.normalize` -- collapse separators, resolve `.` and `..` textually.
pub fn normalize(path: &str) -> String {
    if path.is_empty() {
        return ".".into();
    }

    let (root, rest, absolute) = split_root(path);
    // Node preserves a trailing separator, but not for a bare root.
    let trailing = path.len() > 1 && path.ends_with(is_sep);

    let mut parts: Vec<&str> = Vec::new();
    for segment in rest.split(is_sep) {
        match segment {
            "" | "." => {}
            ".." => match parts.last() {
                Some(&last) if last != ".." => {
                    parts.pop();
                }
                // `..` cannot escape a root; a relative path keeps it.
                _ if absolute => {}
                _ => parts.push(".."),
            },
            other => parts.push(other),
        }
    }

    let body = parts.join(&sep().to_string());
    let mut out = String::from(root);
    if absolute {
        out.push(sep());
    }
    if body.is_empty() {
        // `C:\..\..` is `C:\`; `/..` is `/`; `a/..` is `.`.
        if out.is_empty() {
            return if trailing { "./".into() } else { ".".into() };
        }
        return out;
    }
    out.push_str(&body);
    if trailing {
        out.push(sep());
    }
    out
}

/// Split a path into its root, the remainder, and whether the root is absolute.
///
/// Windows has four kinds of root, and `..` cannot climb past any of them:
///
/// * a drive, `C:\` (and `C:x` is drive-relative, so *not* absolute)
/// * a UNC share, `\\server\share` -- both the server and the share are part
///   of the root
/// * a device namespace, `\\?\` or `\\.\` -- here only the `\\?` is the
///   root, so in `\\?\C:\a` the `C:` is an ordinary poppable segment
/// * a plain rooted path, `\a`
///
/// The UNC and device forms both need *two* segments after the leading `\\`.
/// `\\server` and `\\?\` fall back to being plain rooted paths, matching Node.
fn split_root(path: &str) -> (&str, &str, bool) {
    if WINDOWS {
        if has_drive(path) {
            let after = &path[2..];
            return match after.starts_with(is_sep) {
                true => (&path[..2], &after[1..], true),
                false => (&path[..2], after, false),
            };
        }
        if let Some(split) = split_unc_or_device(path) {
            return split;
        }
    }
    match path.starts_with(is_sep) {
        true => ("", &path[1..], true),
        false => ("", path, false),
    }
}

/// `\\server\share\rest` or `\\?\rest`, when they are well formed.
fn split_unc_or_device(path: &str) -> Option<(&str, &str, bool)> {
    // Two leading separators, in either spelling.
    let after_slashes = path
        .strip_prefix(r"\\")
        .or_else(|| path.strip_prefix("//"))
        .or_else(|| path.strip_prefix(r"\/"))
        .or_else(|| path.strip_prefix(r"/\"))?;

    let mut segments = after_slashes.split(is_sep).filter(|s| !s.is_empty());
    let first = segments.next()?;
    // Both forms need a second segment; otherwise this is just a rooted path.
    segments.next()?;

    // A device namespace roots at `\\?` alone, so everything after it -- even a
    // drive letter -- is an ordinary segment.
    let root_segments = match first {
        "?" | "." => 1,
        _ => 2,
    };

    // Walk past `root_segments` segments in the original string, so the root
    // keeps its exact spelling.
    let mut offset = 2;
    let bytes = after_slashes.as_bytes();
    let mut index = 0;
    for _ in 0..root_segments {
        while index < bytes.len() && is_sep(bytes[index] as char) {
            index += 1;
        }
        while index < bytes.len() && !is_sep(bytes[index] as char) {
            index += 1;
        }
    }
    offset += index;

    let rest = path[offset..].strip_prefix(is_sep).unwrap_or("");
    Some((&path[..offset], rest, true))
}

/// `path.resolve(base, path)` -- make `path` absolute against `base` if it is not
/// already absolute.
pub fn resolve(base: &str, path: &str) -> String {
    if path.starts_with(is_sep) || (WINDOWS && has_drive(path)) {
        normalize(path)
    } else {
        join(base, path)
    }
}

/// `path.relative(from, to)` -- the lexical route from one path to the other,
/// producing `..` segments where needed.
///
/// Node resolves both operands against the cwd first, so a relative `to` comes
/// back unchanged rather than prefixed with a pile of `..`.
pub fn relative(from: &str, to: &str) -> String {
    let from_norm = resolve(from, from);
    let to_norm = resolve(from, to);

    // There is no route between two different roots -- a different drive or a
    // different share -- so Node just hands back the destination.
    let (from_root, _, _) = split_root(&from_norm);
    let (to_root, _, _) = split_root(&to_norm);
    if !same_segment(from_root, to_root) {
        return to_norm;
    }

    let from_parts = segments(&from_norm);
    let to_parts = segments(&to_norm);

    let shared = from_parts
        .iter()
        .zip(&to_parts)
        .take_while(|(a, b)| same_segment(a, b))
        .count();

    let mut parts: Vec<&str> = vec![".."; from_parts.len() - shared];
    parts.extend(to_parts[shared..].iter().copied());
    parts.join(&sep().to_string())
}

fn segments(path: &str) -> Vec<&str> {
    path.split(is_sep).filter(|s| !s.is_empty()).collect()
}

fn same_segment(a: &str, b: &str) -> bool {
    // Windows path comparison is case-insensitive.
    if WINDOWS {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// UNC shares and device namespaces, recorded from node v23 `path.win32`.
    ///
    /// A roaming Windows profile on a network share makes this reachable: before
    /// this was handled, `\\\\server\\share` normalised to `\\server` and every
    /// `~`-relative path resolved somewhere else entirely.
    #[test]
    #[cfg(windows)]
    fn matches_node_win32_unc() {
        let cases: &[(&str, &str)] = &[
            (r"\\", r"\"),
            (r"\\server", r"\server"),
            (r"\\server\", "\\server\\"),
            (r"\\server\share", "\\\\server\\share\\"),
            (r"\\server\share\", "\\\\server\\share\\"),
            (r"\\server\share\a", r"\\server\share\a"),
            (r"\\server\share\..", "\\\\server\\share\\"),
            (r"\\server\share\a\..", "\\\\server\\share\\"),
            (r"\\server\share\a\..\..", "\\\\server\\share\\"),
            (r"\\server\share\a\..\..\..\b", r"\\server\share\b"),
            (r"\\a\b\..\..\..\..\c", r"\\a\b\c"),
            (r"\\?\", "\\?\\"),
            (r"\\?\C:", r"\\?\C:"),
            (r"\\?\C:\..", "\\\\?\\"),
            (r"\\?\C:\a\..", r"\\?\C:"),
            (r"\\?\C:\a\..\..", "\\\\?\\"),
            (r"\\?\UNC\server\share\a\..\..", r"\\?\UNC\server"),
            (r"\\.\", r"\"),
            (r"\\.\pipe\x\..\..", "\\\\.\\"),
            (r"C:a\b", r"C:a\b"),
            (r"\a\..\..", r"\"),
        ];
        let mut bad = 0;
        for (input, want) in cases {
            let got = normalize(input);
            if got != *want {
                println!("norm {input:?} => {got:?}, want {want:?}");
                bad += 1;
            }
        }
        for (a, b, want) in [
            (r"\\server\share", "/.env", r"\\server\share\.env"),
            (r"\\server\share", "/../x", r"\\server\share\x"),
            (r"\\server\share\home", "/../x", r"\\server\share\x"),
        ] {
            let got = join(a, b);
            if got != want {
                println!("MISMATCH join {a:?}+{b:?} => {got:?} want {want:?}");
                bad += 1;
            }
        }
        for (a, b, want) in [
            (r"\\server\share\a", r"\\server\share\b", r"..\b"),
            (r"\\server\share\a\b", r"\\server\share\a\b\c", "c"),
            (r"\\server\share\a", r"C:\b", r"C:\b"),
        ] {
            let got = relative(a, b);
            if got != want {
                println!("MISMATCH rel {a:?}->{b:?} => {got:?} want {want:?}");
                bad += 1;
            }
        }
        assert_eq!(bad, 0, "{bad} divergences from node path.win32");
    }

    // Expectations recorded from node v23 `path.posix`.
    // Expectations recorded from node v23 `path.posix`.
    #[test]
    #[cfg(unix)]
    fn matches_node_posix() {
        assert_eq!(join("/home/u", ""), "/home/u");
        assert_eq!(join("/home/u", "/.env"), "/home/u/.env");
        assert_eq!(join("/home/u", "/../target.env"), "/home/target.env");
        assert_eq!(join("/home/u", "//a"), "/home/u/a");
        assert_eq!(join("/home/u", "./a"), "/home/u/a");
        // A backslash is an ordinary filename character on POSIX.
        assert_eq!(join("/home/u", "\\a.env"), "/home/u/\\a.env");
        assert_eq!(normalize("/a/b/../c"), "/a/c");
        assert_eq!(normalize("/../.."), "/");
        assert_eq!(normalize("a/../.."), "..");
        assert_eq!(normalize("/a/b/"), "/a/b/");

        assert_eq!(relative("/a/b", "/a/b/c"), "c");
        assert_eq!(relative("/a/b/c", "/a/d"), "../../d");
        assert_eq!(relative("/a/b", "/a/b"), "");
        assert_eq!(relative("/tmp/work", "/outside.env"), "../../outside.env");
        // A relative `to` is resolved against `from` first.
        assert_eq!(relative("/tmp/work", "a.env"), "a.env");
        assert_eq!(relative("/tmp/work", "./a.env"), "a.env");
        assert_eq!(relative("/tmp/work", "../a.env"), "../a.env");
        assert_eq!(resolve("/tmp/work", "a.env"), "/tmp/work/a.env");
        assert_eq!(resolve("/tmp/work", "/abs"), "/abs");
    }

    // Expectations recorded from node v23 `path.win32`.
    // Expectations recorded from node v23 `path.win32`.
    #[test]
    #[cfg(windows)]
    fn matches_node_win32() {
        assert_eq!(join(r"C:\home\u", ""), r"C:\home\u");
        assert_eq!(join(r"C:\home\u", "/.env"), r"C:\home\u\.env");
        // Unlike POSIX, a backslash IS a separator here.
        assert_eq!(join(r"C:\home\u", r"\a.env"), r"C:\home\u\a.env");
        assert_eq!(join(r"C:\home\u", "/../target.env"), r"C:\home\target.env");
        assert_eq!(join(r"C:\home\u", "//a"), r"C:\home\u\a");
        assert_eq!(join(r"C:\home\u", "./a"), r"C:\home\u\a");

        assert_eq!(normalize(r"C:\a\b\..\c"), r"C:\a\c");
        // A drive letter is a root: `..` cannot climb past it.
        assert_eq!(normalize(r"C:\..\.."), r"C:\");
        assert_eq!(normalize(r"C:\a\..\..\b"), r"C:\b");
        assert_eq!(normalize(r"C:\a\b\"), "C:\\a\\b\\");
        assert_eq!(normalize(r"a\..\.."), "..");

        assert_eq!(relative(r"C:\a\b", r"C:\a\b\c"), "c");
        assert_eq!(relative(r"C:\a\b\c", r"C:\a\d"), r"..\..\d");
        assert_eq!(
            relative(r"C:\tmp\work", r"C:\outside.env"),
            r"..\..\outside.env"
        );
        // Windows path comparison is case-insensitive.
        assert_eq!(relative(r"C:\A\b", r"C:\a\b\c"), "c");
    }
}
