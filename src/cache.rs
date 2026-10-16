//! Disposable, content-addressed build artifacts. Failed reads and writes
//! are cache misses, never build failures. Executable identity includes
//! templates, parser/index versions and the cache schema's implementation.
use serde::{Serialize, de::DeserializeOwned};
use std::{
    cell::RefCell,
    collections::HashSet,
    fs,
    io::Write,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Default)]
struct Store {
    dir: PathBuf,
    used: RefCell<HashSet<PathBuf>>,
    #[cfg(test)]
    hits: RefCell<std::collections::HashMap<&'static str, usize>>,
}

thread_local! {
    static ACTIVE: RefCell<Option<Option<Rc<Store>>>> = const { RefCell::new(None) };
}

pub struct Session {
    previous: Option<Option<Rc<Store>>>,
    store: Option<Rc<Store>>,
}

impl Session {
    pub fn new(root: &Path, enabled: bool) -> Self {
        static VERSION: OnceLock<Option<String>> = OnceLock::new();
        let version = VERSION.get_or_init(|| {
            fs::read(std::env::current_exe().ok()?)
                .ok()
                .map(|bytes| blake3::hash(&bytes).to_hex().to_string())
        });
        let store = enabled.then_some(version.as_ref()).flatten().map(|version| {
            Rc::new(Store {
                dir: root.join(".trail-cache").join(format!("v1-{version}")),
                ..Store::default()
            })
        });
        let previous = ACTIVE.with(|active| active.replace(Some(store.clone())));
        Self { previous, store }
    }

    /// Retain only entries used by this successful build, including its
    /// cache-busting copy. Concurrent builders may cause harmless misses.
    pub fn finish(&self) {
        let Some(store) = &self.store else { return };
        let Ok(kinds) = fs::read_dir(&store.dir) else {
            return;
        };
        let used = store.used.borrow();
        for kind in kinds.flatten() {
            if !kind.file_type().is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let Ok(entries) = fs::read_dir(kind.path()) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "cache") && !used.contains(&path) {
                    let _ = fs::remove_file(path);
                }
            }
        }
    }

    #[cfg(test)]
    pub fn hits(&self, kind: &str) -> usize {
        self.store
            .as_ref()
            .and_then(|s| s.hits.borrow().get(kind).copied())
            .unwrap_or(0)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        ACTIVE.with(|active| {
            active.replace(self.previous.take());
        });
    }
}

pub fn enabled() -> bool {
    ACTIVE.with(|active| !matches!(&*active.borrow(), Some(None)))
}

pub fn persistent() -> bool {
    store().is_some()
}

fn store() -> Option<Rc<Store>> {
    ACTIVE.with(|active| active.borrow().as_ref().cloned().flatten())
}

pub fn key_json(value: &impl Serialize) -> String {
    // Value's object maps are ordered, including nested maps and dependency
    // indexes originally held in HashMaps.
    let value = serde_json::to_value(value).expect("cache input is serializable");
    crate::memo::key(&[&serde_json::to_vec(&value).expect("JSON value serializes")])
}

fn path(store: &Store, kind: &str, key: &str) -> Option<PathBuf> {
    (key.len() == 64 && key.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| store.dir.join(kind).join(format!("{key}.cache")))
}

pub fn read_bytes(kind: &'static str, key: &str) -> Option<Vec<u8>> {
    let store = store()?;
    let path = path(&store, kind, key)?;
    let bytes = fs::read(&path).ok()?;
    let (digest, payload) = bytes.split_at_checked(32)?;
    if blake3::hash(payload).as_bytes() != digest {
        return None;
    }
    store.used.borrow_mut().insert(path);
    #[cfg(test)]
    {
        *store.hits.borrow_mut().entry(kind).or_default() += 1;
    }
    Some(payload.to_vec())
}

pub fn write_bytes(kind: &'static str, key: &str, bytes: &[u8]) {
    let Some(store) = store() else { return };
    let Some(path) = path(&store, kind, key) else {
        return;
    };
    store.used.borrow_mut().insert(path.clone());
    let Some(parent) = path.parent() else { return };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let temporary = parent.join(format!(
        ".{}-{}.tmp",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(blake3::hash(bytes).as_bytes())?;
        file.write_all(bytes)?;
        drop(file);
        fs::rename(&temporary, &path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
}

pub fn get<T: DeserializeOwned>(kind: &'static str, key: &str) -> Option<T> {
    serde_json::from_slice(&read_bytes(kind, key)?).ok()
}

pub fn put(kind: &'static str, key: &str, value: &impl Serialize) {
    if !persistent() {
        return;
    }
    if let Ok(bytes) = serde_json::to_vec(value) {
        write_bytes(kind, key, &bytes);
    }
}

/// Reuse small, context-independent metadata without retaining parsed events.
pub fn compute<T: Serialize + DeserializeOwned>(kind: &'static str, key: &str, build: impl FnOnce() -> T) -> T {
    if let Some(value) = get(kind, key) { return value; }
    let value = build();
    put(kind, key, &value);
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrupt_unwritable_and_disabled_caches_are_misses() {
        let root = tempfile::tempdir().unwrap();
        let key = crate::memo::key(&[b"input"]);
        {
            let session = Session::new(root.path(), true);
            put("test", &key, &"value");
            assert_eq!(get::<String>("test", &key).as_deref(), Some("value"));
            let path = path(session.store.as_ref().unwrap(), "test", &key).unwrap();
            fs::write(path, b"broken").unwrap();
            assert!(get::<String>("test", &key).is_none());
            assert!(read_bytes("test", "../../outside").is_none());
        }
        let _session = Session::new(root.path(), false);
        put("test", &key, &"disabled");
        assert!(get::<String>("test", &key).is_none());
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join(".trail-cache"), "not a directory").unwrap();
        let _session = Session::new(root.path(), true);
        put("test", &key, &"unwritable");
        assert!(get::<String>("test", &key).is_none());
    }

    #[test]
    fn successful_builds_prune_obsolete_entries_only() {
        let root = tempfile::tempdir().unwrap();
        let old = crate::memo::key(&[b"old"]);
        let new = crate::memo::key(&[b"new"]);
        {
            let _s = Session::new(root.path(), true);
            put("test", &old, &1);
        }
        {
            let s = Session::new(root.path(), true);
            put("test", &new, &2);
            s.finish();
        }
        let _s = Session::new(root.path(), true);
        assert_eq!(get::<i32>("test", &old), None);
        assert_eq!(get::<i32>("test", &new), Some(2));
    }
}
