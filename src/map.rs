//! An insertion-ordered string map.
//!
//! The reference implementation accumulates into a plain JavaScript object, so
//! `parse()`'s result, `populate()`'s iteration order and the debug output are all
//! in file order. A `HashMap` would randomise them per process, so callers that
//! re-serialise a `.env` file or diff debug logs would see shuffled output.
//!
//! Re-assigning an existing key keeps its original position and replaces the
//! value, which is what property assignment does in JavaScript.
//!
//! Enumeration order is not plain insertion order. JavaScript yields *array
//! index* keys first, in ascending numeric order, and only then the remaining
//! keys in insertion order. `dotenv` keys are `[\w.-]+`, which permits all-digit
//! keys, so `ZED=1 / 2=two / AAA=3` enumerates as `2, ZED, AAA`.

use std::collections::HashMap;
use std::ops::Index;

#[derive(Clone, Default)]
pub struct EnvMap {
    entries: Vec<(String, String)>,
    index: HashMap<String, usize>,
}

impl EnvMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace, returning the previous value. Position is preserved on replace.
    pub fn insert(&mut self, key: String, value: String) -> Option<String> {
        match self.index.get(&key) {
            Some(&at) => Some(std::mem::replace(&mut self.entries[at].1, value)),
            None => {
                self.index.insert(key.clone(), self.entries.len());
                self.entries.push((key, value));
                None
            }
        }
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        self.index.get(key).map(|&at| &self.entries[at].1)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.index.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Entries in JavaScript enumeration order: array-index keys ascending, then
    /// the rest in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> + '_ {
        self.order()
            .into_iter()
            .map(move |at| (&self.entries[at].0, &self.entries[at].1))
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> + '_ {
        self.iter().map(|(k, _)| k)
    }

    pub fn values(&self) -> impl Iterator<Item = &String> + '_ {
        self.iter().map(|(_, v)| v)
    }

    /// Positions into `entries`, in enumeration order.
    fn order(&self) -> Vec<usize> {
        let mut indexed: Vec<(u32, usize)> = Vec::new();
        let mut rest: Vec<usize> = Vec::new();

        for (at, (key, _)) in self.entries.iter().enumerate() {
            match array_index(key) {
                Some(n) => indexed.push((n, at)),
                None => rest.push(at),
            }
        }

        if indexed.is_empty() {
            return rest;
        }
        indexed.sort_unstable();
        let mut order: Vec<usize> = indexed.into_iter().map(|(_, at)| at).collect();
        order.extend(rest);
        order
    }
}

/// A JavaScript array index: the canonical decimal form of an integer in
/// `0..=2^32-2`. Only these keys are hoisted to the front of enumeration.
///
/// Canonical means no leading zero (`"01"` is an ordinary key), no sign, no
/// whitespace and no exponent, and `"4294967295"` is one past the limit.
fn array_index(key: &str) -> Option<u32> {
    let bytes = key.as_bytes();
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    if bytes.len() > 1 && bytes[0] == b'0' {
        return None;
    }
    // Longer than u32::MAX's digit count cannot be in range, and would overflow.
    let n: u64 = key.parse().ok()?;
    (n < u32::MAX as u64).then_some(n as u32)
}

impl Index<&str> for EnvMap {
    type Output = String;

    fn index(&self, key: &str) -> &String {
        self.get(key).expect("no such key")
    }
}

impl<'a> IntoIterator for &'a EnvMap {
    type Item = (&'a String, &'a String);
    type IntoIter = Box<dyn Iterator<Item = (&'a String, &'a String)> + 'a>;

    fn into_iter(self) -> Self::IntoIter {
        Box::new(self.iter())
    }
}

impl IntoIterator for EnvMap {
    type Item = (String, String);
    type IntoIter = std::vec::IntoIter<(String, String)>;

    fn into_iter(self) -> Self::IntoIter {
        let order = self.order();
        let mut slots: Vec<Option<(String, String)>> = self.entries.into_iter().map(Some).collect();
        let ordered: Vec<(String, String)> = order
            .into_iter()
            .map(|at| slots[at].take().expect("each position visited once"))
            .collect();
        ordered.into_iter()
    }
}

impl FromIterator<(String, String)> for EnvMap {
    fn from_iter<I: IntoIterator<Item = (String, String)>>(iter: I) -> Self {
        let mut map = EnvMap::new();
        for (key, value) in iter {
            map.insert(key, value);
        }
        map
    }
}

impl Extend<(String, String)> for EnvMap {
    fn extend<I: IntoIterator<Item = (String, String)>>(&mut self, iter: I) {
        for (key, value) in iter {
            self.insert(key, value);
        }
    }
}

/// Order-insensitive, like comparing two JavaScript objects by their entries.
impl PartialEq for EnvMap {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().all(|(k, v)| other.get(k) == Some(v))
    }
}

impl Eq for EnvMap {}

impl std::fmt::Debug for EnvMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn of(pairs: &[(&str, &str)]) -> EnvMap {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn replace_keeps_position_like_a_js_object() {
        let map = of(&[("Z", "1"), ("M", "2"), ("A", "3"), ("M", "4")]);
        let order: Vec<&str> = map.keys().map(String::as_str).collect();
        assert_eq!(order, ["Z", "M", "A"]);
        assert_eq!(map["M"], "4");
        assert_eq!(map.len(), 3);
    }

    #[test]
    fn insert_reports_the_previous_value() {
        let mut map = EnvMap::new();
        assert_eq!(map.insert("K".into(), "a".into()), None);
        assert_eq!(map.insert("K".into(), "b".into()), Some("a".to_string()));
        assert_eq!(map["K"], "b");
    }

    /// The side index must stay in step with the entry list, or lookups start
    /// returning the wrong value.
    #[test]
    fn index_stays_consistent() {
        let mut map = EnvMap::new();
        for round in 0..3 {
            for i in 0..50 {
                map.insert(format!("K{i}"), format!("{round}-{i}"));
            }
        }
        assert_eq!(map.len(), 50);
        for i in 0..50 {
            let key = format!("K{i}");
            assert!(map.contains_key(&key));
            assert_eq!(map.get(&key).unwrap(), &format!("2-{i}"));
        }
        let order: Vec<String> = map.keys().cloned().collect();
        let expected: Vec<String> = (0..50).map(|i| format!("K{i}")).collect();
        assert_eq!(order, expected);
    }

    #[test]
    fn equality_ignores_order() {
        assert_eq!(of(&[("A", "1"), ("B", "2")]), of(&[("B", "2"), ("A", "1")]));
        assert_ne!(of(&[("A", "1")]), of(&[("A", "2")]));
        assert_ne!(of(&[("A", "1")]), of(&[("A", "1"), ("B", "2")]));
    }

    #[test]
    fn empty_and_missing() {
        let map = EnvMap::new();
        assert!(map.is_empty());
        assert_eq!(map.get("nope"), None);
        assert!(!map.contains_key("nope"));
        assert_eq!(map.iter().count(), 0);
    }
}
