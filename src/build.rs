use std::cell::RefCell;
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

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
    rendered: std::collections::HashMap<String, Result<markdown::Rendered>>,
}

/// How a build behaves, beyond the site itself.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct BuildOptions {
    /// Inject the dev-server auto-reload script into every page;
    /// plain `trail build` output never carries it.
    pub live_reload: bool,
    /// Downgrade missing ~link targets from errors to warnings.
    pub allow_dangling_links: bool,
    /// Fail when an article has no `description:` frontmatter and warn
    /// about authored site-absolute links.
    pub strict: bool,
    /// Emit llms-full.txt copies of the print.md bundles.
    pub render_llms_full: bool,
    /// Prefix every link this build emits with this path — "" for the
    /// site itself, "/<cbpath>" for the cache-busting copy. It never
    /// affects where a file is written: the copy is built into its own
    /// output directory, so a page's path stays the page's path and only
    /// the URLs pointing at it grow the prefix.
    pub base: String,
    /// The cache-busting copy's path, if one is being published. Both the
    /// site and the copy carry it, so the /cb command works from either.
    pub cachebust: Option<String>,
}

impl BuildOptions {
    /// The options for building the cache-busting copy of a site built
    /// with these ones: same behaviour, every link prefixed.
    fn cachebust_copy(&self, cbpath: &str) -> BuildOptions {
        BuildOptions {
            base: format!("/{cbpath}"),
            ..self.clone()
        }
    }
}

/// Build the site, and — when one is configured — its cache-busting copy.
/// Returns the number of pages in the site itself; the copy has the same
/// number and is reported separately.
pub fn build_all(site: &Site, out: &Path, options: &BuildOptions) -> Result<usize> {
    if let Some(cbpath) = &options.cachebust {
        validate_cbpath(cbpath, site)?;
    }
    let prepared = Prepared::new(site)?;
    let pages = build_prepared(site, out, options, &prepared)?;
    if let Some(cbpath) = &options.cachebust {
        build_prepared(
            site,
            &out.join(cbpath),
            &options.cachebust_copy(cbpath),
            &prepared,
        )?;
    }
    Ok(pages)
}

/// A cache-busting path has to be a single plain path segment that does
/// not collide with anything the site itself publishes at the top level —
/// otherwise the copy would bury a product, an asset directory, or the
/// search bundle. Checked against the site rather than the output
/// directory so a stale `dist/` cannot make a good path look taken.
fn validate_cbpath(cbpath: &str, site: &Site) -> Result<()> {
    ensure!(!cbpath.is_empty(), "--cbpath cannot be empty");
    ensure!(
        !cbpath.contains('/'),
        "--cbpath '{cbpath}' must be a single path segment, with no '/'"
    );
    ensure!(
        !cbpath.starts_with('.'),
        "--cbpath '{cbpath}' cannot start with a dot"
    );
    ensure!(
        cbpath
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')),
        "--cbpath '{cbpath}' must be made of letters, digits, '-', '_' and '.'"
    );
    let mut taken: Vec<String> = vec![
        "index.html".into(),
        "404.html".into(),
        "assets".into(),
        "pagefind".into(),
        "llms.txt".into(),
        "site.json".into(),
        "robots.txt".into(),
        "sitemap.xml".into(),
    ];
    if site.trees.iter().any(|tree| !tree.attachments.is_empty()) {
        taken.push("attachments".into());
    }
    for product in &site.products {
        taken.push(product.slug.clone());
        taken.push(format!("{}.md", product.slug));
    }
    taken.extend(
        [
            &site.config.custom_css,
            &site.config.favicon,
            &site.config.head_html,
        ]
        .into_iter()
        .flatten()
        .chain(site.trees.iter().flat_map(|tree| &tree.passthrough))
        .filter_map(|path| Path::new(path).components().next())
        .map(|component| component.as_os_str().to_string_lossy().into_owned()),
    );
    ensure!(
        !taken.iter().any(|name| name == cbpath),
        "--cbpath '{cbpath}' is already a path in the site; \
         the copy would bury it — pick a name the site does not use"
    );
    Ok(())
}

pub fn run(args: &BuildArgs) -> Result<()> {
    let cache = crate::cache::Session::new(&args.root, !args.no_cache);
    let out = args.out_dir();
    let site = Site::load(&args.root, &out)?;
    let options = BuildOptions {
        live_reload: false,
        allow_dangling_links: args.allow_dangling_links,
        strict: args.strict,
        render_llms_full: args.render_llms_full,
        base: String::new(),
        cachebust: args.cbpath.clone(),
    };
    let pages = build_all(&site, &out, &options)?;
    cache.finish();
    let collections = site.products.len();
    println!(
        "built {} page{} ({} collection{}) → {}",
        pages,
        if pages == 1 { "" } else { "s" },
        collections,
        if collections == 1 { "" } else { "s" },
        out.display()
    );
    if let Some(cbpath) = &args.cbpath {
        println!("cache-busting copy → {}/{}/", out.display(), cbpath);
    }
    Ok(())
}

/// Build the whole site into `out`, returning the number of pages written.
#[cfg(test)]
pub fn build_site(site: &Site, out: &Path, options: &BuildOptions) -> Result<usize> {
    build_prepared(site, out, options, &Prepared::new(site)?)
}

struct Prepared {
    attachments: Vec<crate::attachments::Published>,
    links: LinkIndex,
    images: ImageIndex,
    refs: InlineRefIndex,
}

impl Prepared {
    fn new(site: &Site) -> Result<Self> {
        Ok(Self {
            attachments: crate::attachments::prepare(site)?,
            links: LinkIndex::new(site),
            images: ImageIndex::new(&site.images),
            refs: InlineRefIndex::new(site)?,
        })
    }
}

/// Render independent article bodies on at most four workers. Each worker
/// owns dependency captures and parser/highlighter memory; the publishing pass
/// consumes results in tree order, including errors and advisory diagnostics.
fn render_articles_parallel(
    site: &Site,
    prepared: &Prepared,
    options: &BuildOptions,
) -> std::collections::HashMap<String, Result<markdown::Rendered>> {
    let articles = site.articles();
    let workers = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .min(4);
    if articles.len() < 64 || workers == 1 {
        return Default::default();
    }
    fn anthology_books<'a>(anthology: &'a Anthology, books: &mut Vec<&'a Book>) {
        for item in anthology.items() {
            match item {
                AnthologyItem::Book { book } => books.push(book),
                AnthologyItem::Anthology { anthology } => anthology_books(anthology, books),
                _ => {}
            }
        }
    }
    let mut all_books = Vec::new();
    for product in &site.products {
        for item in product.items() {
            match item {
                ProductItem::Book { book } => all_books.push(book),
                ProductItem::Anthology { anthology } => anthology_books(anthology, &mut all_books),
                _ => {}
            }
        }
    }
    let books: std::collections::HashMap<_, _> = all_books
        .into_iter()
        .flat_map(|book| {
            book.articles()
                .into_iter()
                .map(move |(_, a)| (a.path.as_str(), book))
        })
        .collect();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let shared = crate::cache::share();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..workers {
            let links = prepared.links.clone();
            let refs = prepared.refs.clone();
            let images = prepared.images.clone();
            let (articles, books, next, shared) = (&articles, &books, &next, &shared);
            handles.push(scope.spawn(move || {
                shared.scope(|| {
                    let start = images.usage_start();
                    let mut rendered = Vec::new();
                    loop {
                        let at = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(article) = articles.get(at) else {
                            break;
                        };
                        let book = books.get(article.path.as_str());
                        let result = markdown::render(
                            &article.body,
                            &links,
                            markdown::RenderOptions {
                                allow_dangling: options.allow_dangling_links,
                                strict: options.strict,
                                numbering: article.number.as_deref(),
                                images: Some(ImageScope {
                                    index: &images,
                                    dir: &article.source_dir,
                                }),
                                refs: Some(markdown::RefScope {
                                    index: &refs,
                                    page: &article.path,
                                    book: book.map(|b| b.path.as_str()),
                                    book_short: book.and_then(|b| b.short.as_deref()),
                                }),
                                base: &options.base,
                                ..Default::default()
                            },
                        );
                        rendered.push((article.path.clone(), result));
                    }
                    (rendered, images.usage_since(start))
                })
            }));
        }
        let mut results = std::collections::HashMap::new();
        for handle in handles {
            let (rendered, images) = handle.join().expect("article renderer panicked");
            prepared.images.replay_usage(&images);
            results.extend(rendered);
        }
        results
    })
}

fn build_prepared(
    site: &Site,
    out: &Path,
    options: &BuildOptions,
    prepared: &Prepared,
) -> Result<usize> {
    let out = &Output::new(out);
    let renderer = Renderer::new(options, site)?;
    let Prepared {
        attachments,
        links,
        images,
        refs,
    } = prepared;
    crate::attachments::publish(attachments, out, &options.base)?;
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
    let mut sink = PageSink {
        rendered: render_articles_parallel(site, prepared, options),
        ..PageSink::default()
    };
    for product in &site.products {
        let html = renderer.product(site, product)?;
        write_page(&mut sink, out, options, &product.path, html)?;
        pages += 1;
        for item in product.items() {
            match item {
                ProductItem::Anthology { anthology } => {
                    let (count, mermaid) = write_anthology(
                        &renderer,
                        links,
                        images,
                        refs,
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
                        links,
                        images,
                        refs,
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
                        links,
                        images,
                        refs,
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
    pages += export::write_ai_surface(site, links, images, refs, &renderer, out, options)?;
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
            crate::diagnostic::warning(format!(
                "{}: unused image was not published",
                asset.source.display()
            ));
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
    for product in &site.products {
        if let (Some(icon), Some(href)) = (&product.icon, product.icon_href()) {
            out.write(
                &out.dir().join(href.trim_start_matches('/')),
                icon.svg.as_bytes(),
            )?;
        }
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
    // Last, so a passthrough named after something trail generates
    // replaces it rather than being quietly overwritten by it. Merged
    // trees go in reverse and the site root last, so where two trees pass
    // the same path through, the root's copy wins, then the first listed.
    for tree in site.trees.iter().rev() {
        for entry in &tree.passthrough {
            copy_passthrough(&tree.root.join(entry), &out.dir().join(entry), out)
                .with_context(|| format!("copying passthrough '{entry}'"))?;
        }
    }
    // The copy has its own tracked writer and pruning pass. Preserve it
    // until then so unchanged copy files keep their bytes and mtimes.
    let copy = options
        .cachebust
        .as_ref()
        .filter(|_| options.base.is_empty())
        .map(|path| out.dir().join(path));
    out.prune(copy.as_deref())?;
    Ok(pages)
}

#[allow(clippy::too_many_arguments)]
fn write_articles(
    renderer: &Renderer,
    links: &LinkIndex,
    images: &ImageIndex,
    refs: &InlineRefIndex,
    options: &BuildOptions,
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
        let rendered = sink
            .rendered
            .remove(&article.path)
            .unwrap_or_else(|| {
                markdown::render(
                    &article.body,
                    links,
                    markdown::RenderOptions {
                        allow_dangling: options.allow_dangling_links,
                        strict: options.strict,
                        images: Some(ImageScope {
                            index: images,
                            dir: &article.source_dir,
                        }),
                        refs: Some(markdown::RefScope {
                            index: refs,
                            page: &article.path,
                            book: None,
                            book_short: None,
                        }),
                        base: &options.base,
                        ..markdown::RenderOptions::default()
                    },
                )
            })
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
            write_page(sink, out, options, &article.path, html)?;
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
    options: &BuildOptions,
    site: &Site,
    product: &Product,
    parents: &[&Anthology],
    anthology: &Anthology,
    out: &Output,
    sink: &mut PageSink,
) -> Result<(usize, bool)> {
    let html = renderer.anthology(site, product, parents, anthology)?;
    write_page(sink, out, options, &anthology.path, html)?;
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
    options: &BuildOptions,
    site: &Site,
    product: &Product,
    anthologies: &[&Anthology],
    book: &Book,
    out: &Output,
    sink: &mut PageSink,
) -> Result<(usize, bool)> {
    let html = renderer.book(site, product, anthologies, book)?;
    write_page(sink, out, options, &book.path, html)?;
    let mut pages = 1;
    let mut uses_mermaid = false;
    let entries = book.articles();
    let order: Vec<(&str, &str)> = entries
        .iter()
        .map(|(_, article)| (article.path.as_str(), article.title.as_str()))
        .collect();
    for (chapters, article) in &entries {
        let rendered = sink
            .rendered
            .remove(&article.path)
            .unwrap_or_else(|| {
                markdown::render(
                    &article.body,
                    links,
                    markdown::RenderOptions {
                        allow_dangling: options.allow_dangling_links,
                        strict: options.strict,
                        numbering: article.number.as_deref(),
                        images: Some(ImageScope {
                            index: images,
                            dir: &article.source_dir,
                        }),
                        refs: Some(markdown::RefScope {
                            index: refs,
                            page: &article.path,
                            book: Some(&book.path),
                            book_short: book.short.as_deref(),
                        }),
                        base: &options.base,
                        ..markdown::RenderOptions::default()
                    },
                )
            })
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
            write_page(sink, out, options, &article.path, html)?;
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
    options: &BuildOptions,
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
            Err(error) if options.allow_dangling_links => {
                crate::diagnostic::warning(format!("{}: related link: {}", article.path, error))
            }
            Err(error) => sink.broken.push(format!(
                "in article '{}' (related): {}",
                article.path, error
            )),
        }
    }
    related
}

/// Copy one `passthrough` entry into the output, recursing through
/// directories. Everything goes through the build's tracked writer, so
/// passed-through files survive the prune like any other output.
fn copy_passthrough(source: &Path, destination: &Path, out: &Output) -> Result<()> {
    if source.is_dir() {
        for entry in
            fs::read_dir(source).with_context(|| format!("reading {}", source.display()))?
        {
            let entry = entry?;
            copy_passthrough(&entry.path(), &destination.join(entry.file_name()), out)?;
        }
        return Ok(());
    }
    let contents = fs::read(source).with_context(|| format!("reading {}", source.display()))?;
    out.write(destination, &contents)
}

/// Write one page to disk and queue it for search indexing under its
/// canonical URL (the trailing-slash form the server redirects to).
/// Print advisory and allowed-dangling warnings immediately; queue fatal link
/// problems on the sink so the build can report them all together.
fn report_links(
    sink: &mut PageSink,
    article: &crate::site::Article,
    rendered: &markdown::Rendered,
) {
    for warning in rendered.warnings.iter().chain(&rendered.dangling) {
        crate::diagnostic::warning(format!("{}: {}", article.path, warning));
    }
    for problem in &rendered.broken {
        sink.broken
            .push(format!("in article '{}': {}", article.path, problem));
    }
}

fn write_page(
    sink: &mut PageSink,
    out: &Output,
    options: &BuildOptions,
    url_path: &str,
    html: String,
) -> Result<()> {
    out.write(&page_path(out.dir(), url_path), html.as_bytes())?;
    // The file lands at the page's own path inside this build's output;
    // the search result has to link to it through the base, since a copy's
    // index is what its own pages search.
    sink.search.push(SearchPage {
        url: format!("{}{url_path}/", options.base),
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
    compare_buffer: RefCell<Vec<u8>>,
}

impl Output {
    pub(crate) fn new(dir: &Path) -> Output {
        Output {
            dir: dir.to_path_buf(),
            written: RefCell::new(HashSet::new()),
            compare_buffer: RefCell::new(vec![0; 64 * 1024]),
        }
    }

    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    pub(crate) fn write(&self, path: &Path, contents: &[u8]) -> Result<()> {
        self.written.borrow_mut().insert(path.to_path_buf());
        // Cached and freshly generated bytes both pass through this writer,
        // sparing disk writes and keeping mtimes stable for rsync-style deploys.
        if self.matches(path, contents) {
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }
        fs::write(path, contents).with_context(|| format!("writing {}", path.display()))
    }

    fn matches(&self, path: &Path, contents: &[u8]) -> bool {
        let Ok(mut file) = fs::File::open(path) else {
            return false;
        };
        if !file
            .metadata()
            .is_ok_and(|m| m.len() == contents.len() as u64)
        {
            return false;
        }
        let mut buffer = self.compare_buffer.borrow_mut();
        for chunk in contents.chunks(buffer.len()) {
            let bytes = &mut buffer[..chunk.len()];
            if file.read_exact(bytes).is_err() || bytes != chunk {
                return false;
            }
        }
        // Also reject a file that grew while it was being compared.
        matches!(file.read(&mut buffer[..1]), Ok(0))
    }

    /// Delete every file this build didn't write, then any directories
    /// left empty. Called only after a successful build, so a failed one
    /// still leaves the previous output intact for the dev server.
    fn prune(&self, preserved: Option<&Path>) -> Result<()> {
        fn walk(dir: &Path, written: &HashSet<PathBuf>, preserved: Option<&Path>) -> Result<bool> {
            let mut empty = true;
            for entry in fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
                let path = entry?.path();
                if preserved == Some(path.as_path()) {
                    empty = false;
                    continue;
                }
                if path.is_dir() {
                    if walk(&path, written, preserved)? {
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
            walk(&self.dir, &self.written.borrow(), preserved)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site;

    fn output_hashes(root: &Path) -> std::collections::BTreeMap<PathBuf, String> {
        fn walk(root: &Path, dir: &Path, result: &mut std::collections::BTreeMap<PathBuf, String>) {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(root, &path, result);
                } else {
                    result.insert(
                        path.strip_prefix(root).unwrap().to_owned(),
                        blake3::hash(&fs::read(path).unwrap()).to_hex().to_string(),
                    );
                }
            }
        }
        let mut result = std::collections::BTreeMap::new();
        walk(root, root, &mut result);
        result
    }

    #[test]
    fn output_comparison_repairs_same_size_edits_and_truncation() {
        let dir = tempfile::tempdir().unwrap();
        let output = Output::new(dir.path());
        let path = dir.path().join("large.html");
        let bytes = vec![b'a'; 200_000];
        output.write(&path, &bytes).unwrap();
        let time = fs::metadata(&path).unwrap().modified().unwrap();
        output.write(&path, &bytes).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), time);
        let mut changed = bytes.clone();
        changed[90_000] = b'b';
        fs::write(&path, changed).unwrap();
        fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_modified(time)
            .unwrap();
        output.write(&path, &bytes).unwrap();
        assert_eq!(fs::read(&path).unwrap(), bytes);
        fs::write(&path, &bytes[..100]).unwrap();
        output.write(&path, &bytes).unwrap();
        assert_eq!(fs::read(&path).unwrap(), bytes);
        fs::remove_file(&path).unwrap();
        output.write(&path, &[]).unwrap();
        assert!(fs::read(&path).unwrap().is_empty());
    }

    #[test]
    fn parallel_article_results_match_serial_with_ordered_diagnostics() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        site::write_fixture(root);
        let topic = root.join("alpha.product/5--loose.topic");
        for n in 100..180 {
            fs::write(topic.join(format!("{n}--parallel-{n}.md")), format!("---\ntitle: Page {n}\ntype: concept\n---\n\n## Heading\n\n[Missing](~alpha/missing) and [Absolute](/absolute).\n\n```rust\nfn main() {{}}\n```\n")).unwrap();
        }
        let _session = crate::cache::Session::new(root, false);
        let site = Site::load(root, &root.join("dist")).unwrap();
        let prepared = Prepared::new(&site).unwrap();
        let options = BuildOptions {
            strict: true,
            allow_dangling_links: true,
            ..Default::default()
        };
        let mut results = render_articles_parallel(&site, &prepared, &options);
        if std::thread::available_parallelism().map_or(1, |n| n.get()) == 1 {
            return;
        }
        for article in site
            .articles()
            .into_iter()
            .filter(|a| a.slug.starts_with("parallel-"))
        {
            let serial = markdown::render(
                &article.body,
                &prepared.links,
                markdown::RenderOptions {
                    allow_dangling: true,
                    strict: true,
                    images: Some(ImageScope {
                        index: &prepared.images,
                        dir: &article.source_dir,
                    }),
                    refs: Some(markdown::RefScope {
                        index: &prepared.refs,
                        page: &article.path,
                        book: None,
                        book_short: None,
                    }),
                    ..Default::default()
                },
            )
            .unwrap();
            let parallel = results.remove(&article.path).unwrap().unwrap();
            assert_eq!(
                serde_json::to_value(parallel).unwrap(),
                serde_json::to_value(serial).unwrap()
            );
        }
    }

    #[test]
    fn incremental_builds_match_uncached_after_content_and_structure_changes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("site");
        let out = dir.path().join("incremental");
        let clean = dir.path().join("clean");
        site::write_fixture(&root);
        let run = |enabled: bool, options: &BuildOptions, destination: &Path| {
            let cache = crate::cache::Session::new(&root, enabled);
            let site = Site::load(&root, destination).unwrap();
            let pages = build_all(&site, destination, options).unwrap();
            let hits = (
                cache.hits("render"),
                cache.hits("search"),
                cache.hits("pages"),
                cache.hits("bundles-html"),
            );
            cache.finish();
            (pages, hits)
        };
        let check = |options: &BuildOptions| {
            let (pages, hits) = run(true, options, &out);
            assert_eq!(pages, run(false, options, &clean).0);
            assert_eq!(output_hashes(&out), output_hashes(&clean));
            hits
        };
        let options = BuildOptions::default();
        check(&options);
        let hits = check(&options);
        assert!(
            hits.0 > 0 && hits.1 > 0,
            "unchanged build must reuse rendering and search"
        );
        assert!(
            hits.2 > 0 && hits.3 > 0,
            "unchanged build must reuse pages and complete bundles"
        );
        let article = root.join("alpha.product/5--loose.topic/1--x1.md");
        let append = |path: &Path, text: &str| {
            let mut source = fs::read_to_string(path).unwrap();
            source.push_str(text);
            fs::write(path, source).unwrap();
        };
        append(&article, "\nChanged body with **emphasis** and `$x$`.\n");
        assert!(
            check(&options).0 > 0,
            "body edits must preserve other render hits"
        );
        append(
            &article,
            &"Longer prose changes aggregate reading times. ".repeat(200),
        );
        let edited = check(&options);
        assert!(
            edited.2 > 0 && edited.3 > 0,
            "unrelated pages and bundles survive changed totals"
        );
        append(&article, "\n## A New Anchor\n");
        check(&options);
        let config = root.join("alpha.product/7--manual.book/trail.toml");
        fs::write(
            &config,
            fs::read_to_string(&config)
                .unwrap()
                .replace("Alpha Manual", "Renamed Manual")
                .replace("AM", "RM"),
        )
        .unwrap();
        check(&options);
        let alias = root.join("alpha.product/5--loose.topic/8--copy.link");
        fs::write(&alias, "target = \"~alpha/x2\"\ntitle = \"Alias title\"\n").unwrap();
        check(&options);
        fs::remove_file(&alias).unwrap();
        let added = article.with_file_name("98--added.md");
        fs::write(
            &added,
            "---\ntitle: Added\ntype: concept\n---\n\nNew content.\n",
        )
        .unwrap();
        check(&options);
        fs::rename(&added, added.with_file_name("98--renamed.md")).unwrap();
        check(&options);
        fs::remove_file(added.with_file_name("98--renamed.md")).unwrap();
        check(&options);
        // Changed dimensions/content and removing all image references must
        // update published assets, including when other pages are cached.
        let image = root.join("alpha.product/7--manual.book/layout.png");
        fs::write(&image, b"changed image content").unwrap();
        check(&options);
        for name in ["1--intro.md", "2--setup/1--install.md"] {
            let path = root.join("alpha.product/7--manual.book").join(name);
            let text = fs::read_to_string(&path)
                .unwrap()
                .lines()
                .filter(|l| !l.starts_with("!["))
                .collect::<Vec<_>>()
                .join("\n");
            fs::write(path, text).unwrap();
        }
        check(&options);
        assert!(!out.join("alpha/manual/layout.png").exists());
        let copy = BuildOptions {
            cachebust: Some("revision-2".into()),
            render_llms_full: true,
            ..Default::default()
        };
        check(&copy);
        let copy_file = out.join("revision-2/index.html");
        let modified = fs::metadata(&copy_file).unwrap().modified().unwrap();
        check(&copy);
        assert_eq!(
            modified,
            fs::metadata(&copy_file).unwrap().modified().unwrap()
        );
        // Removing the copy and llms-full must prune their cached outputs.
        check(&options);
        assert!(!out.join("revision-2").exists());
        let live = BuildOptions {
            live_reload: true,
            ..Default::default()
        };
        check(&live);
        let extra = dir.path().join("extra");
        let topic = extra.join("gamma.product/1--guide.topic");
        fs::create_dir_all(&topic).unwrap();
        fs::write(
            extra.join("gamma.product/trail.toml"),
            "title = \"Gamma\"\nmonogram = \"G\"\ncolor = \"#3b82f6\"\ndescription = \"d\"\n",
        )
        .unwrap();
        let merged = topic.join("1--start.md");
        fs::write(
            &merged,
            "---\ntitle: Start\ntype: concept\n---\n\nMerged content.\n",
        )
        .unwrap();
        append(
            &root.join("trail.toml"),
            "\n[[merge]]\npath = \"../extra\"\n",
        );
        check(&options);
        append(&merged, "\nChanged merged body.\n");
        check(&options);
        // A corrupt search blob must rebuild the entire bundle, including
        // fragments which were otherwise present in the output directory.
        let namespace = fs::read_dir(root.join(".trail-cache"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        let blob = fs::read_dir(namespace.join("search-files"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        fs::write(&blob, b"corrupt").unwrap();
        check(&options);
        assert_ne!(fs::read(blob).unwrap(), b"corrupt");
    }

    #[test]
    fn cached_links_do_not_hide_new_ambiguity_or_strict_errors() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("site");
        let out = dir.path().join("out");
        site::write_fixture(&root);
        let page = root.join("alpha.product/5--loose.topic/1--x1.md");
        fs::write(
            &page,
            "---\ntitle: Link\ntype: concept\n---\n\n[Target](~alpha/b1)\n",
        )
        .unwrap();
        let run = |cache, options: &BuildOptions| {
            let _session = crate::cache::Session::new(&root, cache);
            let site = Site::load(&root, &out).unwrap();
            build_all(&site, &out, options).map_err(|error| format!("{error:#}"))
        };
        run(true, &BuildOptions::default()).unwrap();
        let target = root.join("alpha.product/2--acorn.antho/200--narrow.topic/1--b1.md");
        let original = fs::read_to_string(&target).unwrap();
        fs::write(&target, format!("{original}\n## Target Anchor\n")).unwrap();
        fs::write(
            &page,
            "---\ntitle: Link\ntype: concept\n---\n\n[Target](~alpha/b1#target-anchor)\n",
        )
        .unwrap();
        run(true, &BuildOptions::default()).unwrap();
        fs::write(&target, &original).unwrap();
        let missing = run(true, &BuildOptions::default()).unwrap_err();
        assert_eq!(missing, run(false, &BuildOptions::default()).unwrap_err());
        let allow = BuildOptions {
            allow_dangling_links: true,
            ..Default::default()
        };
        run(true, &allow).unwrap();
        assert_eq!(run(true, &BuildOptions::default()).unwrap_err(), missing);
        fs::write(
            &page,
            "---\ntitle: Link\ntype: concept\n---\n\n[Target](~alpha/b1)\n",
        )
        .unwrap();
        let strict = BuildOptions {
            strict: true,
            ..Default::default()
        };
        assert_eq!(
            run(true, &strict).unwrap_err(),
            run(false, &strict).unwrap_err()
        );
        fs::write(
            root.join("alpha.product/5--loose.topic/99--b1.md"),
            "---\ntitle: Another\ntype: concept\n---\n\nText\n",
        )
        .unwrap();
        let options = BuildOptions {
            allow_dangling_links: true,
            ..Default::default()
        };
        let error = run(true, &options).unwrap_err();
        assert!(error.contains("ambiguous"));
        assert_eq!(error, run(false, &options).unwrap_err());
    }

    #[test]
    fn build_publishes_product_icons_and_draws_them_in_place_of_monograms() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        let product = dir.path().join("alpha.product");
        let config = fs::read_to_string(product.join("trail.toml")).unwrap();
        fs::write(
            product.join("trail.toml"),
            format!("{config}icon = \"mark.svg\"\n"),
        )
        .unwrap();
        fs::write(
            product.join("mark.svg"),
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 24 24\"><path d=\"M0 0h1v1z\"/></svg>",
        )
        .unwrap();
        let out = dir.path().join("dist");
        let site = Site::load(dir.path(), &out).unwrap();
        build_site(&site, &out, &BuildOptions::default()).unwrap();

        let icon = fs::read_to_string(out.join("assets/icons/alpha.svg")).unwrap();
        assert!(icon.starts_with("<svg id=\"trail-icon\" xmlns="), "{icon}");
        let used = "<use href=\"/assets/icons/alpha.svg#trail-icon\"";
        for page in ["index.html", "alpha/index.html", "beta/index.html"] {
            let html = fs::read_to_string(out.join(page)).unwrap();
            assert!(html.contains(used), "{page} draws the icon");
        }
        // The product hero uses it too, and the featured product, which has
        // no icon, keeps its monogram tile on its front-page card.
        let front = fs::read_to_string(out.join("index.html")).unwrap();
        assert!(front.contains("<span class=\"tile\">Xx</span>"));
        let hero = fs::read_to_string(out.join("alpha/index.html")).unwrap();
        assert!(hero.contains("tile tile-hero tile-icon"));
        let json = fs::read_to_string(out.join("site.json")).unwrap();
        assert!(
            json.contains("\"icon\": \"/assets/icons/alpha.svg\""),
            "{json}"
        );
    }

    #[test]
    fn build_writes_the_front_page_and_its_assets() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        let out = dir.path().join("dist");
        let site = Site::load(dir.path(), &out).unwrap();

        build_site(
            &site,
            &out,
            &BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
                ..BuildOptions::default()
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
            &BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
                ..BuildOptions::default()
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
            &BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
                ..BuildOptions::default()
            },
        )
        .unwrap_err();
        assert!(err.to_string().contains("does not match any section"));

        // Downgradable, like any dangling reference.
        let site = Site::load(dir.path(), &out).unwrap();
        build_site(
            &site,
            &out,
            &BuildOptions {
                live_reload: false,
                allow_dangling_links: true,
                strict: false,
                render_llms_full: false,
                ..BuildOptions::default()
            },
        )
        .unwrap();
    }

    #[test]
    fn bad_anchors_and_relative_paths_break_the_build() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        fs::write(
            dir.path().join("alpha.product/5--loose.topic/1--x1.md"),
            "---\ntitle: Article x1\ntype: concept\ndescription: about x1\n---\n\n\
             See [a2](~alpha/a2#nope), [x2](../2--x2.md) and [below](#nowhere).\n",
        )
        .unwrap();
        let out = dir.path().join("dist");
        let site = Site::load(dir.path(), &out).unwrap();
        let err = build_site(
            &site,
            &out,
            &BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
                ..BuildOptions::default()
            },
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("names anchor '#nope'"), "{err}");
        assert!(err.contains("'../2--x2.md' is a relative path"), "{err}");
        assert!(
            err.contains("'#nowhere' names an anchor this page does not have"),
            "{err}"
        );

        // Downgradable, like any dangling reference.
        let site = Site::load(dir.path(), &out).unwrap();
        build_site(
            &site,
            &out,
            &BuildOptions {
                live_reload: false,
                allow_dangling_links: true,
                strict: false,
                render_llms_full: false,
                ..BuildOptions::default()
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
            &BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
                ..BuildOptions::default()
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
            &BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
                ..BuildOptions::default()
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
        let options = &BuildOptions {
            live_reload: false,
            allow_dangling_links: false,
            strict: false,
            render_llms_full: false,
            ..BuildOptions::default()
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
            &BuildOptions {
                allow_dangling_links: true,
                ..options.clone()
            },
        )
        .unwrap_err();
        let message = format!("{err:#}");
        assert!(!message.contains("~alpha/nope"));
        assert!(message.contains("link '~alpha/a1' is ambiguous"));
    }

    #[test]
    fn the_cachebust_copy_is_the_whole_site_with_every_link_prefixed() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        let out = dir.path().join("dist");
        let site = Site::load(dir.path(), &out).unwrap();
        build_all(
            &site,
            &out,
            &BuildOptions {
                cachebust: Some("deadbeef".into()),
                ..BuildOptions::default()
            },
        )
        .unwrap();

        // The copy is a whole site, not a landing page: pages, mirrors,
        // bundles, assets and its own search index.
        assert!(out.join("deadbeef/index.html").is_file());
        assert!(
            out.join("deadbeef/alpha/acorn/wide/a1/index.html")
                .is_file()
        );
        assert!(out.join("deadbeef/alpha/acorn/wide/a1.md").is_file());
        assert!(out.join("deadbeef/alpha/print/index.html").is_file());
        assert!(out.join("deadbeef/assets/style.css").is_file());
        assert!(out.join("deadbeef/pagefind/pagefind.js").is_file());
        assert!(out.join("deadbeef/llms.txt").is_file());
        assert!(out.join("deadbeef/site.json").is_file());

        let copy = fs::read_to_string(out.join("deadbeef/alpha/acorn/wide/a1/index.html")).unwrap();
        // Every link inside the copy stays inside it — the whole point:
        // one that escaped would be served from the cache the copy exists
        // to defeat.
        assert!(copy.contains("href=\"/deadbeef/assets/style.css\""));
        assert!(copy.contains("src=\"/deadbeef/assets/search.js\""));
        assert!(copy.contains("href=\"/deadbeef/alpha\""));
        assert!(copy.contains("href=\"/deadbeef/alpha/acorn/wide/a1.md\""));
        assert!(!copy.contains("href=\"/alpha\""));
        assert!(!copy.contains("href=\"/assets/"));
        assert!(!copy.contains("src=\"/assets/"));
        // The canonical still names the real page, and the duplicate is
        // kept out of search indexes.
        assert!(copy.contains("<link rel=\"canonical\" href=\"/alpha/acorn/wide/a1/\">"));
        assert!(copy.contains("name=\"robots\" content=\"noindex"));
        // Crawler instructions belong to the site, not to a copy of it.
        assert!(!out.join("deadbeef/robots.txt").exists());
        assert!(!out.join("deadbeef/sitemap.xml").exists());

        // The site itself is untouched apart from learning where the copy
        // is, which is what its /cb command needs.
        let real = fs::read_to_string(out.join("alpha/acorn/wide/a1/index.html")).unwrap();
        assert!(real.contains("href=\"/assets/style.css\""));
        assert!(!real.contains("/deadbeef/"));
        assert!(!real.contains("noindex"));
        assert!(real.contains(r#""cachebust":"deadbeef""#));
        assert!(copy.contains(r#""base":"/deadbeef""#));

        // The mirrors an agent follows are prefixed too, or the first
        // link would walk it straight back out of the copy.
        let mirror = fs::read_to_string(out.join("deadbeef/alpha.md")).unwrap();
        assert!(mirror.contains("(/deadbeef/alpha/print.md)"));
        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(out.join("deadbeef/site.json")).unwrap())
                .unwrap();
        let alpha = json["products"]
            .as_array()
            .unwrap()
            .iter()
            .find(|product| product["slug"] == "alpha")
            .unwrap();
        assert_eq!(alpha["path"], "/deadbeef/alpha");
        assert_eq!(alpha["md"], "/deadbeef/alpha.md");
        assert_eq!(alpha["items"][0]["path"], "/deadbeef/alpha/acorn");
        assert_eq!(json["llms"], "/deadbeef/llms.txt");
        let llms = fs::read_to_string(out.join("deadbeef/llms.txt")).unwrap();
        assert!(llms.contains("(/deadbeef/alpha/print.md)"));
        assert!(llms.contains("/deadbeef/site.json"));

        // The sweep that matters: not one URL anywhere in the copy points
        // outside it. Spot-checks above name the surfaces a person thinks
        // of; this catches the one nobody did.
        let mut escaped = Vec::new();
        for file in walk_files(&out.join("deadbeef")) {
            let Ok(text) = fs::read_to_string(&file) else {
                continue;
            };
            let name = file.strip_prefix(&out).unwrap().display().to_string();
            let markers: &[(&str, &str)] = if name.ends_with(".html") {
                &[("href=\"", "\""), ("src=\"", "\"")]
            } else if name.ends_with(".md") || name.ends_with(".txt") {
                &[("](", ")")]
            } else {
                continue;
            };
            for (open, close) in markers {
                for (at, _) in text.match_indices(open) {
                    let from = at + open.len();
                    let Some(end) = text[from..].find(close) else {
                        continue;
                    };
                    let url = &text[from..from + end];
                    if !url.starts_with('/') || url.starts_with("/deadbeef/") {
                        continue;
                    }
                    // The one deliberate exception: a copy's page names the
                    // real page as its canonical.
                    if text[..at].ends_with("<link rel=\"canonical\" ") {
                        continue;
                    }
                    escaped.push(format!("{name}: {url}"));
                }
            }
        }
        escaped.sort();
        escaped.dedup();
        assert!(
            escaped.is_empty(),
            "links escaping the copy:\n{}",
            escaped.join("\n")
        );
    }

    /// Every file under a directory, recursively.
    fn walk_files(dir: &Path) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let Ok(entries) = fs::read_dir(dir) else {
            return files;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(walk_files(&path));
            } else {
                files.push(path);
            }
        }
        files
    }

    #[test]
    fn a_cachebust_path_cannot_bury_the_site_it_copies() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        let out = dir.path().join("dist");
        let site = Site::load(dir.path(), &out).unwrap();
        let refused = |cbpath: &str| validate_cbpath(cbpath, &site).unwrap_err().to_string();
        // A product, its mirror, and the generated directories are all
        // real paths the copy would sit on top of.
        assert!(refused("alpha").contains("already a path in the site"));
        assert!(refused("alpha.md").contains("already a path in the site"));
        assert!(refused("assets").contains("already a path in the site"));
        assert!(refused("pagefind").contains("already a path in the site"));
        assert!(refused("site.json").contains("already a path in the site"));
        // And it has to be one plain segment.
        assert!(refused("").contains("cannot be empty"));
        assert!(refused("a/b").contains("single path segment"));
        assert!(refused(".well-known").contains("cannot start with a dot"));
        assert!(refused("a b").contains("letters, digits"));
        // A commit hash, which is what this is for.
        validate_cbpath("0a1b2c3d4e5f", &site).unwrap();
    }

    #[test]
    fn passthrough_entries_are_copied_and_survive_the_prune() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        let config = fs::read_to_string(dir.path().join("trail.toml")).unwrap();
        fs::write(
            dir.path().join("trail.toml"),
            // robots.txt is one trail generates itself: naming it as a
            // passthrough has to replace the generated one, not lose to it.
            format!("passthrough = [\"CNAME\", \".well-known\", \"robots.txt\"]\n{config}"),
        )
        .unwrap();
        fs::write(dir.path().join("CNAME"), "learn.example.org\n").unwrap();
        fs::write(
            dir.path().join("robots.txt"),
            "User-agent: *\nDisallow: /x\n",
        )
        .unwrap();
        fs::create_dir_all(dir.path().join(".well-known")).unwrap();
        fs::write(
            dir.path().join(".well-known/security.txt"),
            "Contact: mailto:security@example.org\n",
        )
        .unwrap();

        let out = dir.path().join("dist");
        let options = &BuildOptions {
            live_reload: false,
            allow_dangling_links: false,
            strict: false,
            render_llms_full: false,
            ..BuildOptions::default()
        };
        let site = Site::load(dir.path(), &out).unwrap();
        build_site(&site, &out, options).unwrap();

        assert_eq!(
            fs::read_to_string(out.join("CNAME")).unwrap(),
            "learn.example.org\n"
        );
        // Directories are copied through, nesting included.
        assert_eq!(
            fs::read_to_string(out.join(".well-known/security.txt")).unwrap(),
            "Contact: mailto:security@example.org\n"
        );
        // The author's file wins over the generated one.
        assert_eq!(
            fs::read_to_string(out.join("robots.txt")).unwrap(),
            "User-agent: *\nDisallow: /x\n"
        );

        // A second build must not prune what the first passed through.
        let site = Site::load(dir.path(), &out).unwrap();
        build_site(&site, &out, options).unwrap();
        assert!(out.join("CNAME").exists());
        assert!(out.join(".well-known/security.txt").exists());
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
        let options = &BuildOptions {
            live_reload: false,
            allow_dangling_links: false,
            strict: false,
            render_llms_full: false,
            ..BuildOptions::default()
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
        let options = &BuildOptions {
            live_reload: false,
            allow_dangling_links: false,
            strict: false,
            render_llms_full: false,
            ..BuildOptions::default()
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
        let strict = &BuildOptions {
            live_reload: false,
            allow_dangling_links: true,
            strict: true,
            render_llms_full: false,
            ..BuildOptions::default()
        };
        let err = build_site(&site, &out, strict).unwrap_err().to_string();
        assert!(err.contains("without a description"));
        // Described articles pass; bare book articles are named.
        assert!(!err.contains("'/alpha/manual/intro'"));
        assert!(!err.contains("'/alpha/loose/x1'"));
        assert!(err.contains("'/alpha/manual/setup/install'"));

        // The default build doesn't care.
        let lax = &BuildOptions {
            strict: false,
            ..strict.clone()
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
            &BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
                ..BuildOptions::default()
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
            &BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
                ..BuildOptions::default()
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
        let options = &BuildOptions {
            live_reload: false,
            allow_dangling_links: false,
            strict: false,
            render_llms_full: false,
            ..BuildOptions::default()
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
    fn merged_trees_join_the_site_and_keep_their_own_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("site");
        site::write_fixture(&root);
        fs::write(root.join("CNAME"), "site").unwrap();
        let config = fs::read_to_string(root.join("trail.toml")).unwrap();
        fs::write(
            root.join("trail.toml"),
            format!(
                "edit_url = \"https://forge.example/site/{{path}}\"\npassthrough = [\"CNAME\"]\n\
                 {config}\n[[merge]]\npath = \"../extra/\"\n"
            ),
        )
        .unwrap();

        // The merged tree: one product linking into the site's, its own
        // edit links and passthrough, and site settings that are ignored
        // (its nav link would not resolve).
        let extra = dir.path().join("extra");
        let topic = extra.join("gamma.product/1--guide.topic");
        fs::create_dir_all(&topic).unwrap();
        fs::create_dir_all(extra.join("dist/gamma")).unwrap();
        fs::write(extra.join("CNAME"), "extra").unwrap();
        fs::write(extra.join("extra.txt"), "only here").unwrap();
        let extra_config = "sitename = \"Other\"\ntitle = \"Other\"\ndescription = \"d\"\n\
             footer = \"other footer\"\n\
             edit_url = \"https://forge.example/extra/{path}\"\n\
             passthrough = [\"CNAME\", \"extra.txt\"]\n\n\
             [[nav]]\nlabel = \"Gone\"\nurl = \"~nowhere/page\"\n";
        fs::write(extra.join("trail.toml"), extra_config).unwrap();
        fs::write(
            extra.join("gamma.product/trail.toml"),
            "title = \"Gamma\"\nmonogram = \"G\"\ncolor = \"#3b82f6\"\ndescription = \"d\"\n",
        )
        .unwrap();
        fs::write(topic.join("trail.toml"), "title = \"Guide\"\n").unwrap();
        fs::write(
            topic.join("1--start.md"),
            "---\ntitle: Start\ntype: concept\n---\n\nSee [A2](~alpha/a2).\n\n![Figure](fig.png)\n",
        )
        .unwrap();
        fs::write(topic.join("fig.png"), site::TEST_PNG).unwrap();

        let out = root.join("dist");
        let options = &BuildOptions::default();
        let site = Site::load(&root, &out).unwrap();
        build_site(&site, &out, options).unwrap();

        let start = fs::read_to_string(out.join("gamma/guide/start/index.html")).unwrap();
        assert!(start.contains("href=\"/alpha/acorn/wide/a2\""));
        assert!(start.contains("footer text"));
        assert!(
            start.contains("https://forge.example/extra/gamma.product/1--guide.topic/1--start.md")
        );
        // Images resolve beside their page, through the `..` in the path.
        assert_eq!(
            fs::read(out.join("gamma/guide/fig.png")).unwrap(),
            site::TEST_PNG
        );
        let x1 = fs::read_to_string(out.join("alpha/loose/x1/index.html")).unwrap();
        assert!(x1.contains("https://forge.example/site/alpha.product/5--loose.topic/1--x1.md"));
        // Where both trees pass a path through, the site root's copy wins.
        assert_eq!(fs::read_to_string(out.join("CNAME")).unwrap(), "site");
        assert_eq!(
            fs::read_to_string(out.join("extra.txt")).unwrap(),
            "only here"
        );

        // Without a trail.toml the tree still merges, with no edit links.
        fs::remove_file(extra.join("trail.toml")).unwrap();
        fs::remove_file(extra.join("CNAME")).unwrap();
        fs::remove_file(extra.join("extra.txt")).unwrap();
        let site = Site::load(&root, &out).unwrap();
        build_site(&site, &out, options).unwrap();
        let start = fs::read_to_string(out.join("gamma/guide/start/index.html")).unwrap();
        assert!(!start.contains("forge.example"));

        // Matching root collections must agree on their metadata.
        fs::create_dir_all(extra.join("alpha.product")).unwrap();
        fs::copy(
            extra.join("gamma.product/trail.toml"),
            extra.join("alpha.product/trail.toml"),
        )
        .unwrap();
        let err = format!("{:#}", Site::load(&root, &out).unwrap_err());
        assert!(
            err.contains("merge conflict at 'alpha': title differs"),
            "{err}"
        );
        fs::remove_dir_all(extra.join("alpha.product")).unwrap();

        // The merged tree's root is checked like the site root's.
        fs::write(extra.join("stray.txt"), "").unwrap();
        let err = format!("{:#}", Site::load(&root, &out).unwrap_err());
        assert!(err.contains("merging the tree at '../extra/'"), "{err}");
        assert!(err.contains("unexpected file 'stray.txt'"), "{err}");
        fs::remove_file(extra.join("stray.txt")).unwrap();

        // A path that is not a directory fails at load.
        fs::remove_file(root.join("CNAME")).unwrap();
        fs::write(
            root.join("trail.toml"),
            format!("{config}\n[[merge]]\npath = \"../nope\"\n"),
        )
        .unwrap();
        let err = format!("{:#}", Site::load(&root, &out).unwrap_err());
        assert!(
            err.contains("merge path '../nope' is not a directory"),
            "{err}"
        );
    }

    #[test]
    fn alias_pages_declare_their_canonical_original() {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        let out = dir.path().join("dist");
        let options = &BuildOptions {
            live_reload: false,
            allow_dangling_links: false,
            strict: false,
            render_llms_full: false,
            ..BuildOptions::default()
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
            &BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
                ..BuildOptions::default()
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
            &BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
                ..BuildOptions::default()
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
        let options = &BuildOptions {
            live_reload: false,
            allow_dangling_links: false,
            strict: false,
            render_llms_full: false,
            ..BuildOptions::default()
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
            &BuildOptions {
                live_reload: false,
                allow_dangling_links: false,
                strict: false,
                render_llms_full: false,
                ..BuildOptions::default()
            },
        )
        .unwrap();

        assert!(out.join("assets/mermaid.min.js").is_file());
        assert!(out.join("assets/LICENSE-Mermaid.txt").is_file());
    }
}
