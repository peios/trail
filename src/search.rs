use anyhow::{Context, Result};
use pagefind::api::PagefindIndex;

use crate::build::Output;

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
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("starting the search indexing runtime")?;
    let files = runtime.block_on(async move {
        let mut index = PagefindIndex::new(None).context("configuring the search index")?;
        for page in pages {
            index
                .add_html_file(None, Some(page.url.clone()), page.html)
                .await
                .with_context(|| format!("indexing page '{}' for search", page.url))?;
        }
        index
            .get_files()
            .await
            .context("building the search bundle")
    })?;
    let bundle_dir = out.dir().join("pagefind");
    for file in files {
        out.write(&bundle_dir.join(&file.filename), &file.contents)?;
    }
    Ok(())
}
