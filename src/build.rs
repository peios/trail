use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::cli::BuildArgs;
use crate::export;
use crate::highlight;
use crate::images::{ImageIndex, ImageScope};
use crate::links::LinkIndex;
use crate::markdown;
use crate::refs::InlineRefIndex;
use crate::render::{PageLink, Pager, Renderer};
use crate::search::{self, SearchPage};
use crate::site::{
    Anthology, AnthologyItem, Book, Product, ProductItem, Site, Topic, TopicChild, TopicFolder,
};

/// Static assets, compiled into the binary so an installed `trail` is
/// self-contained with no theme directory to locate at runtime.
/// The font licenses ship alongside the fonts — the OFL requires it.
const ASSET_FILES: &[(&str, &[u8])] = &[
    ("assets/style.css", include_bytes!("../theme/style.css")),
    ("assets/search.js", include_bytes!("../theme/search.js")),
    (
        "assets/fonts/InterVariable.woff2",
        include_bytes!("../theme/fonts/InterVariable.woff2"),
    ),
    (
        "assets/fonts/LICENSE-Inter.txt",
        include_bytes!("../theme/fonts/LICENSE-Inter.txt"),
    ),
    (
        "assets/fonts/SpaceGroteskVariable.woff2",
        include_bytes!("../theme/fonts/SpaceGroteskVariable.woff2"),
    ),
    (
        "assets/fonts/LICENSE-SpaceGrotesk.txt",
        include_bytes!("../theme/fonts/LICENSE-SpaceGrotesk.txt"),
    ),
];

/// Only emitted when some article actually contains a mermaid diagram.
const MERMAID_FILES: &[(&str, &[u8])] = &[
    (
        "assets/mermaid.min.js",
        include_bytes!("../theme/vendor/mermaid.min.js"),
    ),
    (
        "assets/LICENSE-Mermaid.txt",
        include_bytes!("../theme/vendor/LICENSE-Mermaid.txt"),
    ),
];

/// Per-page byproducts of the main render pass: pages queued for search
/// indexing, and broken-link reports gathered so one build surfaces every
/// bad link instead of stopping at the first.
#[derive(Default)]
struct PageSink {
    search: Vec<SearchPage>,
    broken: Vec<String>,
}

/// How a build behaves, beyond the site itself.
#[derive(Debug, Clone, Copy)]
pub struct BuildOptions {
    /// Inject the dev-server auto-reload script into every page;
    /// plain `trail build` output never carries it.
    pub live_reload: bool,
    /// Downgrade missing ~link targets from errors to warnings.
    pub allow_dangling_links: bool,
    /// Fail when an article has no `description:` frontmatter.
    pub strict: bool,
    /// Emit llms-full.txt copies of the print.md bundles.
    pub render_llms_full: bool,
}

pub fn run(args: &BuildArgs) -> Result<()> {
    let out = args.out_dir();
    let site = Site::load(&args.root, &out)?;
    let options = BuildOptions {
        live_reload: false,
        allow_dangling_links: args.allow_dangling_links,
        strict: args.strict,
        render_llms_full: args.render_llms_full,
    };
    let pages = build_site(&site, &out, options)?;
    let products = site.products.len();
    println!(
        "built {} page{} ({} product{}) → {}",
        pages,
        if pages == 1 { "" } else { "s" },
        products,
        if products == 1 { "" } else { "s" },
        out.display()
    );
    Ok(())
}

/// Build the whole site into `out`, returning the number of pages written.
pub fn build_site(site: &Site, out: &Path, options: BuildOptions) -> Result<usize> {
    let out = &Output::new(out);
    let renderer = Renderer::new(options.live_reload)?;
    let links = LinkIndex::new(site);
    let images = ImageIndex::new(&site.images);
    let refs = InlineRefIndex::new(site)?;
    out.write(
        &out.dir().join("index.html"),
        renderer.index(site)?.as_bytes(),
    )?;
    if options.strict {
        let missing: Vec<String> = site
            .articles()
            .into_iter()
            .filter(|article| article.original.is_none())
            .filter(|article| article.description.as_deref().is_none_or(str::is_empty))
            .map(|article| format!("  article '{}' has no description", article.path))
            .collect();
        if !missing.is_empty() {
            bail!(
                "--strict: {} article{} without a description:\n{}",
                missing.len(),
                if missing.len() == 1 { "" } else { "s" },
                missing.join("\n")
            );
        }
    }
    out.write(
        &out.dir().join("404.html"),
        renderer.not_found(site)?.as_bytes(),
    )?;
    let mut pages = 2;
    let mut uses_mermaid = false;
    // Every page except the front page is fed to the search indexer; the
    // data-pagefind-* attributes in the templates decide what text counts.
    let mut sink = PageSink::default();
    for product in &site.products {
        let html = renderer.product(site, product)?;
        write_page(&mut sink, out, &product.path, html)?;
        pages += 1;
        for item in product.items() {
            match item {
                ProductItem::Anthology { anthology } => {
                    let (count, mermaid) = write_anthology(
                        &renderer,
                        &links,
                        &images,
                        &refs,
                        options,
                        site,
                        product,
                        &[],
                        anthology,
                        out,
                        &mut sink,
                    )?;
                    pages += count;
                    uses_mermaid |= mermaid;
                }
                ProductItem::Topic { topic } => {
                    let (count, mermaid) = write_articles(
                        &renderer,
                        &links,
                        &images,
                        &refs,
                        options,
                        site,
                        product,
                        &[],
                        topic,
                        out,
                        &mut sink,
                    )?;
                    pages += count;
                    uses_mermaid |= mermaid;
                }
                ProductItem::Book { book } => {
                    let (count, mermaid) = write_book(
                        &renderer,
                        &links,
                        &images,
                        &refs,
                        options,
                        site,
                        product,
                        &[],
                        book,
                        out,
                        &mut sink,
                    )?;
                    pages += count;
                    uses_mermaid |= mermaid;
                }
            }
        }
    }
    if !sink.broken.is_empty() {
        bail!(
            "{} broken link{}:\n{}",
            sink.broken.len(),
            if sink.broken.len() == 1 { "" } else { "s" },
            sink.broken.join("\n")
        );
    }
    // The AI-facing surface: markdown mirrors, /print bundles, llms.txt,
    // site.json, sitemap, robots. Print pages are pages; the rest are not.
    pages += export::write_ai_surface(
        site,
        &links,
        &images,
        &renderer,
        out,
        options.render_llms_full,
    )?;
    // Referenced images ship at their published URLs; an image nothing
    // references stays out of the output — a warning, not an error, so a
    // file added ahead of its article doesn't block the build.
    for asset in &site.images {
        if images.is_used(asset) {
            let contents = fs::read(&asset.source)
                .with_context(|| format!("reading image {}", asset.source.display()))?;
            out.write(
                &out.dir().join(asset.url.trim_start_matches('/')),
                &contents,
            )?;
        } else {
            eprintln!(
                "warning: image '{}' is not referenced by any article and was not published",
                asset.source.display()
            );
        }
    }
    search::write_search_bundle(sink.search, out)?;
    for (rel, contents) in ASSET_FILES {
        out.write(&out.dir().join(rel), contents)?;
    }
    // The bundled syntax definitions carry licences that ask to travel
    // with them, exactly as the fonts' do.
    out.write(
        &out.dir().join("assets/LICENSE-Syntaxes.txt"),
        highlight::licenses().as_bytes(),
    )?;
    if let Some(source) = &site.custom_css {
        let contents = fs::read(source)
            .with_context(|| format!("reading custom stylesheet {}", source.display()))?;
        out.write(&out.dir().join("assets/custom.css"), &contents)?;
    }
    if let (Some(source), Some(href)) = (&site.favicon, site.favicon_href()) {
        let contents =
            fs::read(source).with_context(|| format!("reading favicon {}", source.display()))?;
        out.write(&out.dir().join(href.trim_start_matches('/')), &contents)?;
    }
    if uses_mermaid {
        for (rel, contents) in MERMAID_FILES {
            out.write(&out.dir().join(rel), contents)?;
        }
    }
    out.prune()?;
    Ok(pages)
}

#[allow(clippy::too_many_arguments)]
fn write_articles(
    renderer: &Renderer,
    links: &LinkIndex,
    images: &ImageIndex,
    refs: &InlineRefIndex,
    options: BuildOptions,
    site: &Site,
    product: &Product,
    anthologies: &[&Anthology],
    topic: &Topic,
    out: &Output,
    sink: &mut PageSink,
) -> Result<(usize, bool)> {
    let mut pages = 0;
    let mut uses_mermaid = false;
    // Reading order for the pager: subfolder contents inline, exactly
    // as the sidebar presents them.
    let order: Vec<(&str, &str)> = topic
        .pages()
        .map(|article| (article.path.as_str(), article.title.as_str()))
        .collect();
    let write_one = |folder: Option<&TopicFolder>,
                     article: &crate::site::Article,
                     sink: &mut PageSink|
     -> Result<bool> {
        let rendered = markdown::render(
            &article.body,
            links,
            markdown::RenderOptions {
                allow_dangling: options.allow_dangling_links,
                images: Some(ImageScope {
                    index: images,
                    dir: &article.source_dir,
                }),
                refs: Some(markdown::RefScope {
                    index: refs,
                    page: &article.path,
                    book: None,
                }),
                ..markdown::RenderOptions::default()
            },
        )
        .with_context(|| format!("in article '{}'", article.path))?;
        report_links(sink, article, &rendered);
        let related = resolve_related(article, links, options, sink);
        let html = renderer.article(
            site,
            product,
            anthologies,
            topic,
            folder,
            article,
            &rendered,
            &related,
            pager(&order, article, &topic.path),
        )?;
        if article.original.is_some() {
            // Linked pages render like any other but stay out of the
            // search index — the canonical article covers the content.
            out.write(&page_path(out.dir(), &article.path), html.as_bytes())?;
        } else {
            write_page(sink, out, &article.path, html)?;
        }
        Ok(rendered.has_mermaid)
    };
    for child in &topic.children {
        match child {
            TopicChild::Article { article } => {
                uses_mermaid |= write_one(None, article, sink)?;
                pages += 1;
            }
            TopicChild::Folder { folder } => {
                for article in &folder.articles {
                    uses_mermaid |= write_one(Some(folder), article, sink)?;
                    pages += 1;
                }
            }
        }
    }
    Ok((pages, uses_mermaid))
}

/// Write an anthology's page and everything beneath it — anthologies
/// nest, so this recurses with a growing parent trail (for breadcrumbs).
#[allow(clippy::too_many_arguments)]
fn write_anthology(
    renderer: &Renderer,
    links: &LinkIndex,
    images: &ImageIndex,
    refs: &InlineRefIndex,
    options: BuildOptions,
    site: &Site,
    product: &Product,
    parents: &[&Anthology],
    anthology: &Anthology,
    out: &Output,
    sink: &mut PageSink,
) -> Result<(usize, bool)> {
    let html = renderer.anthology(site, product, parents, anthology)?;
    write_page(sink, out, &anthology.path, html)?;
    let mut pages = 1;
    let mut uses_mermaid = false;
    let mut trail: Vec<&Anthology> = parents.to_vec();
    trail.push(anthology);
    for item in anthology.items() {
        let (count, mermaid) = match item {
            AnthologyItem::Topic { topic } => write_articles(
                renderer, links, images, refs, options, site, product, &trail, topic, out, sink,
            )?,
            AnthologyItem::Book { book } => write_book(
                renderer, links, images, refs, options, site, product, &trail, book, out, sink,
            )?,
            AnthologyItem::Anthology { anthology } => write_anthology(
                renderer, links, images, refs, options, site, product, &trail, anthology, out, sink,
            )?,
        };
        pages += count;
        uses_mermaid |= mermaid;
    }
    Ok((pages, uses_mermaid))
}

/// Write a book's cover page and every article in its tree.
#[allow(clippy::too_many_arguments)]
fn write_book(
    renderer: &Renderer,
    links: &LinkIndex,
    images: &ImageIndex,
    refs: &InlineRefIndex,
    options: BuildOptions,
    site: &Site,
    product: &Product,
    anthologies: &[&Anthology],
    book: &Book,
    out: &Output,
    sink: &mut PageSink,
) -> Result<(usize, bool)> {
    let html = renderer.book(site, product, anthologies, book)?;
    write_page(sink, out, &book.path, html)?;
    let mut pages = 1;
    let mut uses_mermaid = false;
    let entries = book.articles();
    let order: Vec<(&str, &str)> = entries
        .iter()
        .map(|(_, article)| (article.path.as_str(), article.title.as_str()))
        .collect();
    for (chapters, article) in &entries {
        let rendered = markdown::render(
            &article.body,
            links,
            markdown::RenderOptions {
                allow_dangling: options.allow_dangling_links,
                numbering: article.number.as_deref(),
                images: Some(ImageScope {
                    index: images,
                    dir: &article.source_dir,
                }),
                refs: Some(markdown::RefScope {
                    index: refs,
                    page: &article.path,
                    book: Some(&book.path),
                }),
                ..markdown::RenderOptions::default()
            },
        )
        .with_context(|| format!("in article '{}'", article.path))?;
        report_links(sink, article, &rendered);
        uses_mermaid |= rendered.has_mermaid;
        let related = resolve_related(article, links, options, sink);
        let html = renderer.book_article(
            site,
            product,
            anthologies,
            book,
            chapters,
            article,
            &rendered,
            &related,
            pager(&order, article, &book.path),
        )?;
        if article.original.is_some() {
            // Linked pages render like any other but stay out of the
            // search index — the canonical article covers the content.
            out.write(&page_path(out.dir(), &article.path), html.as_bytes())?;
        } else {
            write_page(sink, out, &article.path, html)?;
        }
        pages += 1;
    }
    Ok((pages, uses_mermaid))
}

/// An article's place in its unit: the pages either side of it in
/// reading order, and where it sits in the unit's single-page view.
fn pager(order: &[(&str, &str)], article: &crate::site::Article, unit_path: &str) -> Pager {
    let at = order.iter().position(|(path, _)| *path == article.path);
    let link = |index: Option<usize>| {
        index
            .and_then(|index| order.get(index))
            .map(|(path, title)| PageLink {
                path: path.to_string(),
                title: title.to_string(),
            })
    };
    Pager {
        previous: link(at.and_then(|at| at.checked_sub(1))),
        next: link(at.map(|at| at + 1)),
        print_url: Some(format!(
            "{unit_path}/print#{}",
            export::print_anchor(unit_path, article)
        )),
    }
}

/// Resolve an article's `related:` references for its Related Content
/// list. Failures behave exactly like body links — ambiguity is always
/// fatal, missing targets are fatal unless downgraded — and a reference
/// that doesn't resolve is dropped from the list.
fn resolve_related(
    article: &crate::site::Article,
    links: &LinkIndex,
    options: BuildOptions,
    sink: &mut PageSink,
) -> Vec<PageLink> {
    let mut related = Vec::new();
    for reference in &article.related {
        match links.resolve_page(reference) {
            Ok((path, title)) => related.push(PageLink { path, title }),
            Err(error @ crate::links::ResolveError::Ambiguous(_)) => sink.broken.push(format!(
                "in article '{}' (related): {}",
                article.path, error
            )),
            Err(error) if options.allow_dangling_links => eprintln!(
                "warning: in article '{}' (related): {}",
                article.path, error
            ),
            Err(error) => sink.broken.push(format!(
                "in article '{}' (related): {}",
                article.path, error
            )),
        }
    }
    related
}

/// Write one page to disk and queue it for search indexing under its
/// canonical URL (the trailing-slash form the server redirects to).
/// Print allowed-dangling warnings immediately; queue fatal link
/// problems on the sink so the build can report them all together.
fn report_links(
    sink: &mut PageSink,
    article: &crate::site::Article,
    rendered: &markdown::Rendered,
) {
    for warning in &rendered.dangling {
        eprintln!("warning: in article '{}': {}", article.path, warning);
    }
    for problem in &rendered.broken {
        sink.broken
            .push(format!("in article '{}': {}", article.path, problem));
    }
}

fn write_page(sink: &mut PageSink, out: &Output, url_path: &str, html: String) -> Result<()> {
    out.write(&page_path(out.dir(), url_path), html.as_bytes())?;
    sink.search.push(SearchPage {
        url: format!("{url_path}/"),
        html,
    });
    Ok(())
}

/// Where a page with the given site-absolute URL path lands on disk.
fn page_path(out: &Path, url_path: &str) -> PathBuf {
    out.join(url_path.trim_start_matches('/'))
        .join("index.html")
}

/// The build's output directory, tracking every file written so the
/// build can prune what it didn't produce. The output directory belongs
/// to trail: anything found in it that this build did not write — pages
/// for renamed or deleted content, superseded search fragments, stray
/// files — is deleted at the end of a successful build.
pub(crate) struct Output {
    dir: PathBuf,
    written: RefCell<HashSet<PathBuf>>,
}

impl Output {
    pub(crate) fn new(dir: &Path) -> Output {
        Output {
            dir: dir.to_path_buf(),
            written: RefCell::new(HashSet::new()),
        }
    }

    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    pub(crate) fn write(&self, path: &Path, contents: &[u8]) -> Result<()> {
        self.written.borrow_mut().insert(path.to_path_buf());
        // Unchanged files are left untouched: a cheap rebuild cache that
        // spares disk writes and keeps mtimes stable for rsync-style
        // deploys. (Skipping the *render* of unchanged pages would need
        // real dependency tracking — links reach across the whole site —
        // and builds are nowhere near slow enough to justify it yet.)
        if let Ok(existing) = fs::read(path)
            && existing == contents
        {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }
        fs::write(path, contents).with_context(|| format!("writing {}", path.display()))
    }

    /// Delete every file this build didn't write, then any directories
    /// left empty. Called only after a successful build, so a failed one
    /// still leaves the previous output intact for the dev server.
    fn prune(&self) -> Result<()> {
        fn walk(dir: &Path, written: &HashSet<PathBuf>) -> Result<bool> {
            let mut empty = true;
            for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
                let path = entry?.path();
                if path.is_dir() {
                    if walk(&path, written)? {
                        fs::remove_dir(&path)
                            .with_context(|| format!("removing {}", path.display()))?;
                    } else {
                        empty = false;
                    }
                } else if written.contains(&path) {
                    empty = false;
                } else {
                    fs::remove_file(&path)
                        .with_context(|| format!("removing stale {}", path.display()))?;
                }
            }
            Ok(empty)
        }
        if self.dir.exists() {
            walk(&self.dir, &self.written.borrow())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site;

    #[test]
    fn build_writes_the_front_page_and_its_assets() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        let out = dir.path().join("dist");
        let site = Site::load(dir.path(), &out).unwrap();

        build_site(
            &site,
            &out,
            BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
            },
        )
        .unwrap();

        let html = fs::read_to_string(out.join("index.html")).unwrap();
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("Test Learn"));
        assert!(html.contains("Beta"));
        assert!(out.join("alpha/index.html").is_file());
        assert!(out.join("alpha/acorn/index.html").is_file());
        assert!(out.join("alpha/acorn/wide/a1/index.html").is_file());
        assert!(out.join("alpha/loose/x1/index.html").is_file());
        assert!(out.join("alpha/acorn/narrow/extra/c1/index.html").is_file());
        // Book cover plus articles at every nesting depth.
        assert!(out.join("alpha/manual/index.html").is_file());
        assert!(out.join("alpha/manual/intro/index.html").is_file());
        assert!(
            out.join("alpha/manual/setup/advanced/tuning/index.html")
                .is_file()
        );
        assert!(out.join("beta/index.html").is_file());
        assert!(out.join("assets/style.css").is_file());
        assert!(out.join("assets/search.js").is_file());
        // The AI surface rides along with every build.
        assert!(out.join("llms.txt").is_file());
        assert!(out.join("site.json").is_file());
        assert!(out.join("robots.txt").is_file());
        assert!(out.join("alpha.md").is_file());
        assert!(out.join("alpha/acorn/wide/a1.md").is_file());
        assert!(out.join("alpha/print/index.html").is_file());
        // llms-full.txt copies are opt-in via --render-llms-full.
        assert!(!out.join("alpha/llms-full.txt").exists());
        // The search bundle: engine module plus index chunks.
        assert!(out.join("pagefind/pagefind.js").is_file());
        assert!(out.join("pagefind/pagefind-entry.json").is_file());
        // No article uses mermaid, so the asset stays out of the output.
        assert!(!out.join("assets/mermaid.min.js").exists());
    }

    #[test]
    fn inline_refs_and_rfc_keywords_enrich_prose() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        let out = dir.path().join("dist");
        let site = Site::load(dir.path(), &out).unwrap();
        build_site(
            &site,
            &out,
            BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
            },
        )
        .unwrap();

        // A phrase links to its declarer; a book phrase reaches a
        // section through its § suffix.
        let x1 = fs::read_to_string(out.join("alpha/loose/x1/index.html")).unwrap();
        assert!(x1.contains("<a class=\"inline-ref\" href=\"/alpha/loose/x2\">X-Two</a>"));
        assert!(x1.contains(
            "<a class=\"inline-ref\" href=\"/alpha/manual/setup/advanced/tuning\">\
             Alpha Manual §2.2.1</a>"
        ));

        // A page never links a phrase to itself; RFC keywords highlight;
        // code stays plain.
        let x2 = fs::read_to_string(out.join("alpha/loose/x2/index.html")).unwrap();
        assert!(!x2.contains("href=\"/alpha/loose/x2\">X-Two"));
        assert!(x2.contains("<a class=\"inline-ref\" href=\"/alpha/manual\">Alpha Manual</a>"));
        assert!(x2.contains("<span class=\"rfc-keyword\">MUST NOT</span>"));
        assert!(x2.contains("<span class=\"rfc-keyword\">MAY</span>"));
        assert!(x2.contains("<code>Alpha Manual</code>"));

        // Bare § references resolve within the surrounding book.
        let install =
            fs::read_to_string(out.join("alpha/manual/setup/install/index.html")).unwrap();
        assert!(install.contains(
            "<a class=\"inline-ref\" href=\"/alpha/manual/setup/advanced/tuning\">§2.2.1</a>"
        ));
        assert!(install.contains("<a class=\"inline-ref\" href=\"/alpha/manual/glossary\">§A</a>"));
        // An unresolvable bare § stays plain and silent — prose cites
        // external documents' section numbers all the time.
        assert!(install.contains("§9.9"));
        assert!(!install.contains(">§9.9</a>"));
    }

    #[test]
    fn unresolvable_section_references_break_the_build() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        fs::write(
            dir.path().join("alpha.product/5--loose.topic/1--x1.md"),
            "---\ntitle: Article x1\ntype: concept\ndescription: about x1\n---\n\n\
             Alpha Manual §9.9 is fiction.\n",
        )
        .unwrap();
        let out = dir.path().join("dist");
        let site = Site::load(dir.path(), &out).unwrap();
        let err = build_site(
            &site,
            &out,
            BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("does not match any section"));

        // Downgradable, like any dangling reference.
        let site = Site::load(dir.path(), &out).unwrap();
        build_site(
            &site,
            &out,
            BuildOptions {
                live_reload: false,
                allow_dangling_links: true,
                strict: false,
                render_llms_full: false,
            },
        )
        .unwrap();
    }

    #[test]
    fn referenced_images_ship_and_orphans_stay_home() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        fs::write(
            dir.path().join("alpha.product/5--loose.topic/orphan.png"),
            site::TEST_PNG,
        )
        .unwrap();
        // An alias in another topic renders the pic article's body; its
        // images must still resolve against the original's folder.
        fs::write(
            dir.path()
                .join("alpha.product/2--acorn.antho/200--narrow.topic/5--extra/3--piclink.link"),
            "target = \"~alpha/pic\"\n",
        )
        .unwrap();
        let out = dir.path().join("dist");
        let site = Site::load(dir.path(), &out).unwrap();

        build_site(
            &site,
            &out,
            BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
            },
        )
        .unwrap();

        // Referenced images land at their published URLs, byte for byte.
        assert_eq!(
            fs::read(out.join("alpha/loose/wiring.png")).unwrap(),
            site::TEST_PNG
        );
        assert!(out.join("alpha/loose/glyph.svg").is_file());
        assert!(out.join("alpha/manual/layout.png").is_file());
        assert!(
            !out.join("alpha/loose/orphan.png").exists(),
            "unreferenced images are not published"
        );

        // The page carries the resolved tag: absolute URL, header-read
        // dimensions, and the caption as a real figure.
        let pic = fs::read_to_string(out.join("alpha/loose/pic/index.html")).unwrap();
        assert!(pic.contains(
            "<figure><img src=\"/alpha/loose/wiring.png\" alt=\"Wiring overview\" \
             width=\"2\" height=\"1\"><figcaption>The wiring</figcaption></figure>"
        ));
        // "../" destinations cross into the parent chapter's URL space.
        let install =
            fs::read_to_string(out.join("alpha/manual/setup/install/index.html")).unwrap();
        assert!(install.contains("<img src=\"/alpha/manual/layout.png\""));
        // The alias page shows the same resolved image.
        let alias =
            fs::read_to_string(out.join("alpha/acorn/narrow/extra/piclink/index.html")).unwrap();
        assert!(alias.contains("<img src=\"/alpha/loose/wiring.png\""));
    }

    #[test]
    fn related_content_renders_at_the_foot_of_pages() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        // Book articles carry related too.
        fs::write(
            dir.path().join("alpha.product/7--manual.book/4--refs.md"),
            "---\ntitle: Refs\nrelated:\n  - alpha/x2\n---\n\nBody of refs.\n",
        )
        .unwrap();
        let out = dir.path().join("dist");
        let site = Site::load(dir.path(), &out).unwrap();

        build_site(
            &site,
            &out,
            BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
            },
        )
        .unwrap();

        // Resolved links, in frontmatter order, titled after their targets.
        let a1 = fs::read_to_string(out.join("alpha/acorn/wide/a1/index.html")).unwrap();
        assert!(a1.contains("Related Content"));
        assert!(a1.contains("<a href=\"/alpha/acorn/narrow/b1\">Article b1</a>"));
        assert!(a1.contains("<a href=\"/alpha/manual\">Alpha Manual</a>"));
        let refs = fs::read_to_string(out.join("alpha/manual/refs/index.html")).unwrap();
        assert!(refs.contains("<a href=\"/alpha/loose/x2\">Article x2</a>"));
        // No related, no section.
        let d1 = fs::read_to_string(out.join("alpha/acorn/inner/deep/d1/index.html")).unwrap();
        assert!(!d1.contains("Related Content"));
    }

    #[test]
    fn broken_related_references_break_the_build() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        fs::write(
            dir.path().join("alpha.product/5--loose.topic/22--badrel.md"),
            "---\ntitle: Badrel\ntype: concept\ndescription: d\nrelated:\n  - alpha/nope\n  - alpha/a1\n---\n\nBody.\n",
        )
        .unwrap();
        let out = dir.path().join("dist");
        let site = Site::load(dir.path(), &out).unwrap();
        let options = BuildOptions {
            live_reload: false,
            allow_dangling_links: false,
            strict: false,
            render_llms_full: false,
        };

        let err = build_site(&site, &out, options).unwrap_err();
        let message = format!("{err:#}");
        assert!(message.contains(
            "in article '/alpha/loose/badrel' (related): \
             link '~alpha/nope' matches no page"
        ));
        assert!(message.contains("link '~alpha/a1' is ambiguous"));

        // allow-dangling drops the missing target with a warning, but
        // ambiguity still has to be settled by the author.
        let err = build_site(
            &site,
            &out,
            BuildOptions {
                allow_dangling_links: true,
                ..options
            },
        )
        .unwrap_err();
        let message = format!("{err:#}");
        assert!(!message.contains("~alpha/nope"));
        assert!(message.contains("link '~alpha/a1' is ambiguous"));
    }

    #[test]
    fn theming_flows_into_every_page() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        let config = fs::read_to_string(dir.path().join("trail.toml")).unwrap();
        fs::write(
            dir.path().join("trail.toml"),
            format!("accent = \"#112233\"\ncustom_css = \"custom.css\"\n{config}"),
        )
        .unwrap();
        fs::write(dir.path().join("custom.css"), ":root { --bg: #000 }").unwrap();
        let out = dir.path().join("dist");
        let options = BuildOptions {
            live_reload: false,
            allow_dangling_links: false,
            strict: false,
            render_llms_full: false,
        };
        let site = Site::load(dir.path(), &out).unwrap();
        build_site(&site, &out, options).unwrap();

        let html = fs::read_to_string(out.join("index.html")).unwrap();
        // The configured accent, with its dark variant derived.
        assert!(html.contains(":root { --accent: #112233; }"));
        assert!(html.contains("color-mix(in srgb, #112233 75%, white)"));
        // The custom stylesheet ships and is linked after the built-in one.
        assert!(html.contains("href=\"/assets/custom.css\""));
        assert_eq!(
            fs::read_to_string(out.join("assets/custom.css")).unwrap(),
            ":root { --bg: #000 }"
        );
        // The theme toggle and its pre-paint boot script ride on every page.
        assert!(html.contains("data-theme-toggle"));
        assert!(html.contains("trail-theme"));
        let article = fs::read_to_string(out.join("alpha/loose/x1/index.html")).unwrap();
        assert!(article.contains("data-theme-toggle"));
        // Product theming is on by default — no collapse rule.
        assert!(!html.contains("--pc: var(--accent) !important"));

        // Turning it off collapses every per-product tint to the accent.
        let config = fs::read_to_string(dir.path().join("trail.toml")).unwrap();
        fs::write(
            dir.path().join("trail.toml"),
            format!("product_theming = false\n{config}"),
        )
        .unwrap();
        let site = Site::load(dir.path(), &out).unwrap();
        build_site(&site, &out, options).unwrap();
        let html = fs::read_to_string(out.join("index.html")).unwrap();
        assert!(html.contains("--pc: var(--accent) !important"));
        let config = fs::read_to_string(dir.path().join("trail.toml")).unwrap();
        fs::write(
            dir.path().join("trail.toml"),
            config.replace("product_theming = false\n", ""),
        )
        .unwrap();

        // An explicit accent_dark overrides the derivation.
        let config = fs::read_to_string(dir.path().join("trail.toml")).unwrap();
        fs::write(
            dir.path().join("trail.toml"),
            format!("accent_dark = \"#445566\"\n{config}"),
        )
        .unwrap();
        let site = Site::load(dir.path(), &out).unwrap();
        build_site(&site, &out, options).unwrap();
        let html = fs::read_to_string(out.join("index.html")).unwrap();
        assert!(html.contains(":root[data-theme=\"dark\"] { --accent: #445566; }"));
        assert!(!html.contains("color-mix(in srgb, #112233"));
    }

    #[test]
    fn every_page_carries_its_head_meta() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        let out = dir.path().join("dist");
        let options = BuildOptions {
            live_reload: false,
            allow_dangling_links: false,
            strict: false,
            render_llms_full: false,
        };
        let site = Site::load(dir.path(), &out).unwrap();
        build_site(&site, &out, options).unwrap();

        // The front page describes the site.
        let front = fs::read_to_string(out.join("index.html")).unwrap();
        assert!(front.contains("<meta name=\"description\" content=\"A fixture site.\">"));
        assert!(front.contains("<meta property=\"og:title\" content=\"Test Learn\">"));
        assert!(front.contains("<meta property=\"og:type\" content=\"website\">"));
        assert!(front.contains("<meta name=\"twitter:card\" content=\"summary\">"));
        assert!(front.contains("<link rel=\"canonical\" href=\"/\">"));
        // Without a site url there is no absolute og:url to give.
        assert!(!front.contains("og:url"));

        // Articles describe themselves and are og:type article.
        let x1 = fs::read_to_string(out.join("alpha/loose/x1/index.html")).unwrap();
        assert!(x1.contains("<meta name=\"description\" content=\"about x1\">"));
        assert!(x1.contains("<meta property=\"og:description\" content=\"about x1\">"));
        assert!(x1.contains("<meta property=\"og:title\" content=\"Article x1\">"));
        assert!(x1.contains("<meta property=\"og:type\" content=\"article\">"));
        assert!(x1.contains("<link rel=\"canonical\" href=\"/alpha/loose/x1/\">"));

        // Book articles carry no description by default: the tag is
        // omitted rather than invented. The cover describes the book.
        let intro = fs::read_to_string(out.join("alpha/manual/intro/index.html")).unwrap();
        assert!(!intro.contains("meta name=\"description\""));
        assert!(intro.contains("<meta property=\"og:type\" content=\"article\">"));
        let cover = fs::read_to_string(out.join("alpha/manual/index.html")).unwrap();
        assert!(cover.contains("<meta name=\"description\" content=\"book description\">"));

        // The 404 page ships, asks not to be indexed, and links home.
        let missing = fs::read_to_string(out.join("404.html")).unwrap();
        assert!(missing.contains("<meta name=\"robots\" content=\"noindex\">"));
        assert!(missing.contains("This page doesn't exist."));
        assert!(!missing.contains("rel=\"canonical\""));

        // With a url, canonicals and og:url go absolute.
        let config = fs::read_to_string(dir.path().join("trail.toml")).unwrap();
        fs::write(
            dir.path().join("trail.toml"),
            format!("url = \"https://docs.example\"\n{config}"),
        )
        .unwrap();
        let site = Site::load(dir.path(), &out).unwrap();
        build_site(&site, &out, options).unwrap();
        let x1 = fs::read_to_string(out.join("alpha/loose/x1/index.html")).unwrap();
        assert!(
            x1.contains("<link rel=\"canonical\" href=\"https://docs.example/alpha/loose/x1/\">")
        );
        assert!(x1.contains(
            "<meta property=\"og:url\" content=\"https://docs.example/alpha/loose/x1/\">"
        ));
    }

    #[test]
    fn strict_builds_require_descriptions_everywhere() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        // A book article may carry a description; strict then accepts it.
        fs::write(
            dir.path().join("alpha.product/7--manual.book/1--intro.md"),
            "---\ntitle: Article intro\ndescription: how it starts\n---\n\nBody of intro.\n",
        )
        .unwrap();
        let out = dir.path().join("dist");
        let site = Site::load(dir.path(), &out).unwrap();
        let strict = BuildOptions {
            live_reload: false,
            allow_dangling_links: true,
            strict: true,
            render_llms_full: false,
        };
        let err = build_site(&site, &out, strict).unwrap_err().to_string();
        assert!(err.contains("without a description"));
        // Described articles pass; bare book articles are named.
        assert!(!err.contains("'/alpha/manual/intro'"));
        assert!(!err.contains("'/alpha/loose/x1'"));
        assert!(err.contains("'/alpha/manual/setup/install'"));

        // The default build doesn't care.
        let lax = BuildOptions {
            strict: false,
            ..strict
        };
        build_site(&site, &out, lax).unwrap();
    }

    #[test]
    fn pages_link_to_their_neighbours_and_single_page_view() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        let out = dir.path().join("dist");
        let site = Site::load(dir.path(), &out).unwrap();
        build_site(
            &site,
            &out,
            BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
            },
        )
        .unwrap();

        // A book article sits between its neighbours in reading order,
        // across chapter boundaries.
        let install =
            fs::read_to_string(out.join("alpha/manual/setup/install/index.html")).unwrap();
        assert!(install.contains("<a class=\"pager-previous\" href=\"/alpha/manual/intro\">"));
        assert!(
            install
                .contains("<a class=\"pager-next\" href=\"/alpha/manual/setup/advanced/tuning\">")
        );
        // And offers the whole book on one page, opened at itself.
        assert!(
            install.contains("class=\"print-link\" href=\"/alpha/manual/print#setup-install\"")
        );

        // The first page has no previous; the last has no next.
        let intro = fs::read_to_string(out.join("alpha/manual/intro/index.html")).unwrap();
        assert!(!intro.contains("pager-previous"));
        assert!(intro.contains("pager-next"));

        // Topic articles page through the topic, subfolders inline.
        let c1 = fs::read_to_string(out.join("alpha/acorn/narrow/extra/c1/index.html")).unwrap();
        assert!(c1.contains("<a class=\"pager-previous\" href=\"/alpha/acorn/narrow/b2\">"));
        assert!(c1.contains("class=\"print-link\" href=\"/alpha/acorn/narrow/print#extra-c1\""));
    }

    #[test]
    fn reading_time_and_updated_show_on_pages_and_cards() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        let out = dir.path().join("dist");
        let site = Site::load(dir.path(), &out).unwrap();
        build_site(
            &site,
            &out,
            BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
            },
        )
        .unwrap();

        // A dated article says both; an undated one says only the time.
        let x2 = fs::read_to_string(out.join("alpha/loose/x2/index.html")).unwrap();
        assert!(x2.contains("1 min read"));
        assert!(x2.contains("Updated 2026-03-15"));
        let x1 = fs::read_to_string(out.join("alpha/loose/x1/index.html")).unwrap();
        assert!(x1.contains("1 min read"));
        assert!(!x1.contains("Updated"));

        // Long books read in hours, on the cover and the card.
        let cover = fs::read_to_string(out.join("alpha/manual/index.html")).unwrap();
        assert!(cover.contains("hr"));
        assert!(cover.contains("Updated 2026-05-01"));
        let product = fs::read_to_string(out.join("alpha/index.html")).unwrap();
        assert!(product.contains("class=\"card-meta\""));
        assert!(product.contains("articles ·"));
    }

    #[test]
    fn config_extras_flow_into_the_output() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        fs::write(dir.path().join("favicon.png"), site::TEST_PNG).unwrap();
        fs::write(
            dir.path().join("head.html"),
            "<meta name=\"marker\" content=\"injected\">",
        )
        .unwrap();
        let config = fs::read_to_string(dir.path().join("trail.toml")).unwrap();
        fs::write(
            dir.path().join("trail.toml"),
            format!(
                "favicon = \"favicon.png\"\nhead_html = \"head.html\"\n\
                 edit_url = \"https://forge.example/repo/edit/main/{{path}}\"\n\
                 {config}\n\
                 [[nav]]\nlabel = \"External\"\nurl = \"https://example.com/\"\n\n\
                 [[nav]]\nlabel = \"A2\"\nurl = \"~alpha/a2\"\n"
            ),
        )
        .unwrap();
        let out = dir.path().join("dist");
        let options = BuildOptions {
            live_reload: false,
            allow_dangling_links: false,
            strict: false,
            render_llms_full: false,
        };
        let site = Site::load(dir.path(), &out).unwrap();
        build_site(&site, &out, options).unwrap();

        // The favicon ships at the root under its own name; every page
        // links it and carries the injected head snippet.
        assert!(out.join("favicon.png").is_file());
        let front = fs::read_to_string(out.join("index.html")).unwrap();
        assert!(front.contains("<link rel=\"icon\" href=\"/favicon.png\">"));
        assert!(front.contains("<meta name=\"marker\" content=\"injected\">"));

        // Nav links: external kept, ~reference resolved at load.
        assert!(front.contains("<a class=\"nav-link\" href=\"https://example.com/\">External</a>"));
        assert!(front.contains("<a class=\"nav-link\" href=\"/alpha/acorn/wide/a2\">A2</a>"));
        assert!(front.contains("menu-nav-link"));

        // Edit links point at the true source file — topic articles,
        // book articles, and aliases (which edit their original).
        let x1 = fs::read_to_string(out.join("alpha/loose/x1/index.html")).unwrap();
        assert!(x1.contains(
            "https://forge.example/repo/edit/main/alpha.product/5--loose.topic/1--x1.md"
        ));
        let intro = fs::read_to_string(out.join("alpha/manual/intro/index.html")).unwrap();
        assert!(intro.contains(
            "https://forge.example/repo/edit/main/alpha.product/7--manual.book/1--intro.md"
        ));
        let alias = fs::read_to_string(out.join("alpha/loose/alias/index.html")).unwrap();
        assert!(alias.contains(
            "https://forge.example/repo/edit/main/\
             alpha.product/2--acorn.antho/100--wide.topic/2--a2.md"
        ));

        // Misconfigurations fail at load.
        fs::write(
            dir.path().join("trail.toml"),
            format!(
                "favicon = \"favicon.png\"\nhead_html = \"head.html\"\n\
                 edit_url = \"https://forge.example/edit\"\n{config}"
            ),
        )
        .unwrap();
        let err = Site::load(dir.path(), &out).unwrap_err().to_string();
        assert!(err.contains("{path}"));
        fs::write(
            dir.path().join("trail.toml"),
            format!(
                "favicon = \"favicon.png\"\nhead_html = \"head.html\"\n{config}\n\
                 [[nav]]\nlabel = \"Gone\"\nurl = \"~alpha/nope\"\n"
            ),
        )
        .unwrap();
        let err = Site::load(dir.path(), &out).unwrap_err().to_string();
        assert!(err.contains("resolving nav link 'Gone'"));
    }

    #[test]
    fn alias_pages_declare_their_canonical_original() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        let out = dir.path().join("dist");
        let options = BuildOptions {
            live_reload: false,
            allow_dangling_links: false,
            strict: false,
            render_llms_full: false,
        };
        let site = Site::load(dir.path(), &out).unwrap();
        build_site(&site, &out, options).unwrap();

        // Without a base url canonicals are root-relative: an alias names
        // its original, a real page names itself.
        let alias = fs::read_to_string(out.join("alpha/loose/alias/index.html")).unwrap();
        assert!(alias.contains("<link rel=\"canonical\" href=\"/alpha/acorn/wide/a2/\">"));
        let a2 = fs::read_to_string(out.join("alpha/acorn/wide/a2/index.html")).unwrap();
        assert!(a2.contains("<link rel=\"canonical\" href=\"/alpha/acorn/wide/a2/\">"));

        // A configured base url makes it absolute.
        let config = fs::read_to_string(dir.path().join("trail.toml")).unwrap();
        fs::write(
            dir.path().join("trail.toml"),
            format!("url = \"https://docs.example\"\n{config}"),
        )
        .unwrap();
        let site = Site::load(dir.path(), &out).unwrap();
        build_site(&site, &out, options).unwrap();
        let alias = fs::read_to_string(out.join("alpha/loose/alias/index.html")).unwrap();
        assert!(alias.contains(
            "<link rel=\"canonical\" href=\"https://docs.example/alpha/acorn/wide/a2/\">"
        ));
    }

    #[test]
    fn missing_images_break_the_build() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        fs::write(
            dir.path().join("alpha.product/5--loose.topic/21--noimg.md"),
            "---\ntitle: Noimg\ntype: concept\ndescription: d\n---\n\n![x](gone.png)\n",
        )
        .unwrap();
        let out = dir.path().join("dist");
        let site = Site::load(dir.path(), &out).unwrap();

        let err = build_site(
            &site,
            &out,
            BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
            },
        )
        .unwrap_err();
        let message = format!("{err:#}");
        assert!(message.contains("in article '/alpha/loose/noimg'"));
        assert!(message.contains("image 'gone.png' not found"));
    }

    #[test]
    fn builds_report_every_broken_link_at_once() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        fs::write(
            dir.path().join("alpha.product/5--loose.topic/12--bad1.md"),
            "---\ntitle: Bad1\ntype: concept\ndescription: d\n---\n\n[gone](~alpha/nope)\n",
        )
        .unwrap();
        fs::write(
            dir.path().join("alpha.product/5--loose.topic/13--bad2.md"),
            "---\ntitle: Bad2\ntype: concept\ndescription: d\n---\n\n[which](~alpha/a1)\n",
        )
        .unwrap();
        let out = dir.path().join("dist");
        let site = Site::load(dir.path(), &out).unwrap();

        let err = build_site(
            &site,
            &out,
            BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
            },
        )
        .unwrap_err();

        let message = format!("{err:#}");
        assert!(message.contains("2 broken links"));
        assert!(message.contains("in article '/alpha/loose/bad1': link '~alpha/nope'"));
        assert!(message.contains("in article '/alpha/loose/bad2': link '~alpha/a1'"));
    }

    #[test]
    fn stale_output_is_pruned_on_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        let out = dir.path().join("dist");
        let site = Site::load(dir.path(), &out).unwrap();
        let options = BuildOptions {
            live_reload: false,
            allow_dangling_links: false,
            strict: false,
            render_llms_full: false,
        };
        build_site(&site, &out, options).unwrap();

        // Leftovers from renamed content, and files trail never wrote.
        fs::write(out.join("junk.txt"), "old").unwrap();
        fs::create_dir_all(out.join("ghost/dir")).unwrap();
        fs::write(out.join("ghost/dir/index.html"), "old").unwrap();

        build_site(&site, &out, options).unwrap();
        assert!(!out.join("junk.txt").exists());
        assert!(!out.join("ghost").exists(), "emptied directories go too");
        assert!(out.join("alpha/index.html").is_file());
        assert!(out.join("pagefind/pagefind.js").is_file());
    }

    #[test]
    fn mermaid_asset_ships_only_when_a_diagram_exists() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        fs::write(
            dir.path()
                .join("alpha.product/5--loose.topic/11--diagram.md"),
            "---\ntitle: Diagram\ntype: concept\ndescription: d\n---\n\n```mermaid\ngraph TD;\n```\n",
        )
        .unwrap();
        let out = dir.path().join("dist");
        let site = Site::load(dir.path(), &out).unwrap();

        build_site(
            &site,
            &out,
            BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
            },
        )
        .unwrap();

        assert!(out.join("assets/mermaid.min.js").is_file());
        assert!(out.join("assets/LICENSE-Mermaid.txt").is_file());
    }
}
