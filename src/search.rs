use anyhow::{Context, Result};
use pagefind::api::PagefindIndex;

use crate::build::Output;

#[derive(serde::Serialize, serde::Deserialize)]
struct CachedSearch {
    files: Vec<(std::path::PathBuf, String)>,
    entries: Vec<crate::cache::Entry>,
}

struct CompressionCache { shared: crate::cache::Shared, enabled: bool }

impl pagefind::api::CompressionCache for CompressionCache {
    fn get(&self, source: &[u8]) -> Option<Vec<u8>> {
        if !self.enabled { return None; }
        self.shared.scope(|| crate::cache::read_bytes("search-compressed", &crate::memo::key(&[source])))
    }
    fn put(&self, source: &[u8], encoded: &[u8]) {
        if self.enabled { self.shared.scope(|| crate::cache::write_bytes("search-compressed", &crate::memo::key(&[source]), encoded)); }
    }
}

/// A rendered page queued for search indexing: its canonical URL (with a
/// trailing slash, as the server redirects to) and its full HTML. What of
/// the page actually lands in the index is controlled by data-pagefind-*
/// attributes in the templates.
pub struct SearchPage {
    pub url: String,
    pub html: String,
}

/// Build the Pagefind search bundle for the given pages into
/// `<out>/pagefind/` — the index chunks, the WASM search core, and the
/// `/pagefind/pagefind.js` module the theme's search script imports.
/// The files go through the build's tracked writer so superseded index
/// fragments get pruned like any other stale output.
///
/// Pagefind's indexing API is async; trail's build pipeline is
/// deliberately synchronous, so the indexing runs on a private runtime.
/// `build_site` is never called from inside a tokio runtime (the dev
/// server enters async only after building), so blocking here is safe.
pub fn write_search_bundle(pages: Vec<SearchPage>, out: &Output) -> Result<()> {
    let mut input = blake3::Hasher::new();
    for page in &pages {
        for field in [page.url.as_bytes(), page.html.as_bytes()] {
            input.update(&(field.len() as u64).to_le_bytes());
            input.update(field);
        }
    }
    let key = input.finalize().to_hex().to_string();
    let cached = crate::cache::get::<CachedSearch>("search", &key)
        .and_then(|manifest| {
            crate::cache::retain(&manifest.entries);
            manifest.files.into_iter().map(|(path, hash)| {
            if path.is_absolute() || path.components().any(|c| !matches!(c, std::path::Component::Normal(_))) {
                return None;
            }
            Some((path, crate::cache::read_bytes("search-files", &hash)?))
        }).collect::<Option<Vec<_>>>() });
    if let Some(files) = cached {
        for (path, bytes) in files { out.write(&out.dir().join("pagefind").join(path), &bytes)?; }
        return Ok(());
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("starting the search indexing runtime")?;
    let entries = crate::cache::capture();
    let compression = CompressionCache { shared: crate::cache::share(), enabled: crate::cache::persistent() };
    let files = runtime.block_on(async move {
        let mut index = PagefindIndex::new(None).context("configuring the search index")?;
        let workers = std::thread::available_parallelism().map_or(1, |n| n.get()).min(8);
        index.add_html_files(pages.into_iter().map(|page| (page.url, page.html)).collect(), workers)
            .await.context("indexing pages for search")?;
        index
            .get_files_cached(&compression, workers)
            .await
            .context("building the search bundle")
    })?;
    let entries = entries.finish();
    let bundle_dir = out.dir().join("pagefind");
    let mut manifest = Vec::new();
    for file in files {
        let hash = crate::memo::key(&[&file.contents]);
        crate::cache::write_bytes("search-files", &hash, &file.contents);
        manifest.push((file.filename.clone(), hash));
        out.write(&bundle_dir.join(&file.filename), &file.contents)?;
    }
    crate::cache::put("search", &key, &CachedSearch { files: manifest, entries });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn pages() -> Vec<(String, String)> {
        (0..12).map(|i| (format!("/copy/docs/page-{i}/"), format!(
            "<html lang=\"en\"><head><title>Page {i}</title></head><body><nav data-pagefind-ignore>Ignored</nav><article data-pagefind-body><h1>Title {i}</h1><p>Common words, unique word{i}, &amp; Unicode café.</p></article></body></html>"
        ))).collect()
    }

    #[test]
    fn parallel_batches_match_single_page_index_bytes() {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let mut serial = PagefindIndex::new(None).unwrap();
            for (url, html) in pages() { serial.add_html_file(None, Some(url), html).await.unwrap(); }
            let expected: BTreeMap<_, _> = serial.get_files().await.unwrap().into_iter()
                .map(|f| (f.filename, f.contents)).collect();
            for workers in [1, 4] {
                let mut batch = PagefindIndex::new(None).unwrap();
                batch.add_html_files(pages(), workers).await.unwrap();
                let actual: BTreeMap<_, _> = batch.get_files().await.unwrap().into_iter()
                    .map(|f| (f.filename, f.contents)).collect();
                assert_eq!(actual, expected);
            }
        });
    }

    #[test]
    fn cached_parallel_compression_matches_original_after_edits() {
        use std::sync::{Mutex, atomic::{AtomicUsize, Ordering}};
        #[derive(Default)]
        struct MemoryCache {
            values: Mutex<BTreeMap<Vec<u8>, Vec<u8>>>,
            hits: AtomicUsize,
        }
        impl pagefind::api::CompressionCache for MemoryCache {
            fn get(&self, source: &[u8]) -> Option<Vec<u8>> {
                let value = self.values.lock().unwrap().get(source).cloned();
                if value.is_some() { self.hits.fetch_add(1, Ordering::Relaxed); }
                value
            }
            fn put(&self, source: &[u8], encoded: &[u8]) {
                self.values.lock().unwrap().insert(source.to_vec(), encoded.to_vec());
            }
        }
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let cache = MemoryCache::default();
            let mut input = pages();
            input.push(("/fr/".into(), "<html lang=\"fr\"><body><h1>Bonjour</h1><p>Une autre langue.</p></body></html>".into()));
            for step in 0..4 {
                if step == 2 { input[0].1 = input[0].1.replace("Common words", "Changed words"); }
                if step == 3 {
                    input.remove(1);
                    input.push(("/added/".into(), "<html lang=\"en\"><body><h1>Added page</h1></body></html>".into()));
                }
                let mut index = PagefindIndex::new(None).unwrap();
                index.add_html_files(input.clone(), 4).await.unwrap();
                let expected: BTreeMap<_, _> = index.get_files().await.unwrap().into_iter().map(|f| (f.filename, f.contents)).collect();
                let hits = cache.hits.load(Ordering::Relaxed);
                let actual: BTreeMap<_, _> = index.get_files_cached(&cache, 4).await.unwrap().into_iter().map(|f| (f.filename, f.contents)).collect();
                assert_eq!(actual, expected, "step {step}");
                if step > 0 { assert!(cache.hits.load(Ordering::Relaxed) > hits); }
            }
        });
    }

    #[test]
    fn invalid_batches_do_not_populate_the_index() {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        runtime.block_on(async {
            let mut index = PagefindIndex::new(None).unwrap();
            let page = pages().remove(0);
            assert!(index.add_html_files(vec![page.clone(), page.clone()], 2).await.is_err());
            assert!(index.add_html_files(vec![page.clone()], 0).await.is_err());
            index.add_html_files(vec![page.clone()], 2).await.unwrap();
            assert!(index.add_html_files(vec![page], 2).await.is_err());
        });
    }
}
