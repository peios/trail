//! Bounded, process-local reuse of pure transformations. Keys are content
//! digests; callers account for the retained keys and values in bytes.
use std::collections::{HashMap, VecDeque};

pub struct Memo<T> {
    entries: HashMap<String, (T, usize)>,
    order: VecDeque<String>,
    bytes: usize,
    limit: usize,
}

impl<T: Clone> Memo<T> {
    pub fn new(limit: usize) -> Self {
        Self {
            entries: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            limit,
        }
    }

    pub fn get(&self, key: &str) -> Option<T> {
        self.entries.get(key).map(|(value, _)| value.clone())
    }

    pub fn insert(&mut self, key: String, value: T, bytes: usize) {
        let bytes = bytes.saturating_add(key.len() * 2 + 128);
        if bytes > self.limit || self.entries.contains_key(&key) {
            return;
        }
        while self.bytes + bytes > self.limit {
            if let Some(oldest) = self.order.pop_front() {
                if let Some((_, size)) = self.entries.remove(&oldest) {
                    self.bytes -= size;
                }
            } else {
                break;
            }
        }
        self.bytes += bytes;
        self.order.push_back(key.clone());
        self.entries.insert(key, (value, bytes));
    }
}

/// Length-delimited fields avoid ambiguous concatenations in cache keys.
pub fn key(parts: &[&[u8]]) -> String {
    let mut hasher = blake3::Hasher::new();
    for part in parts {
        hasher.update(&(part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    hasher.finalize().to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eviction_and_oversize_entries_keep_the_budget() {
        let mut memo = Memo::new(400);
        memo.insert("a".into(), 1, 100);
        memo.insert("b".into(), 2, 100);
        assert_eq!(memo.get("a"), None);
        assert_eq!(memo.get("b"), Some(2));
        memo.insert("large".into(), 3, 401);
        assert_eq!(memo.get("large"), None);
        assert!(memo.bytes <= 400);
        assert_ne!(key(&[b"ab", b"c"]), key(&[b"a", b"bc"]));
    }
}
