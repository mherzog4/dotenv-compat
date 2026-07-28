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

    let absolute = path.starts_with(is_sep);
    // Node preserves a trailing separator, but not for a bare root.
    let trailing = path.len() > 1 && path.ends_with(is_sep);

    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split(is_sep) {
        match segment {
            "" | "." => {}
            ".." => match parts.last() {
                Some(&last) if last != ".." => {
                    parts.pop();
                }
                // A `..` that escapes the root is dropped; a relative path keeps it.
                _ if absolute => {}
                _ => parts.push(".."),
            },
            other => parts.push(other),
        }
    }

    let mut out = parts.join(&sep().to_string());
    if out.is_empty() {
        return if absolute {
            sep().to_string()
        } else if trailing {
            "./".into()
        } else {
            ".".into()
        };
    }
    if absolute {
        out.insert(0, sep());
    }
    if trailing {
        out.push(sep());
    }
    out
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

fn has_drive(path: &str) -> bool {
    let b = path.as_bytes();
    b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':'
}

/// `path.relative(from, to)` -- the lexical route from one path to the other,
/// producing `..` segments where needed.
///
/// Node resolves both operands against the cwd first, so a relative `to` comes
/// back unchanged rather than prefixed with a pile of `..`.
pub fn relative(from: &str, to: &str) -> String {
    let from_norm = resolve(from, from);
    let to_norm = resolve(from, to);
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

    // Expectations recorded from node v23 `path.posix`. Only meaningful on POSIX,
    // where our separator handling matches.
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
}
