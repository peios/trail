//! The AI-facing surface of the site: markdown mirrors of every page,
//! whole-unit bundles (`/print`, `/print.md`, and optionally
//! `llms-full.txt`), the root `llms.txt`, the machine-readable
//! `site.json`, and `sitemap.xml` / `robots.txt`. All of it is derived
//! from the loaded model at build time — plain static files, nothing to
//! host beyond the site itself.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

use anyhow::Result;
use serde_json::json;

use crate::build::Output;
use crate::images::{ImageIndex, ImageScope};
use crate::links::LinkIndex;
use crate::markdown;
use crate::render::{PrintSection, Renderer};
use crate::site::{
    Anthology, AnthologyItem, Article, Book, BookChild, Product, ProductItem, Site, Topic,
    TopicChild,
};

/// A bundleable unit: a container whose articles read as one document.
struct Unit<'a> {
    title: String,
    description: Option<&'a str>,
    /// The unit's URL path; bundles live at `<path>/print[.md]`.
    path: &'a str,
    /// Every article in the unit in reading order, with its display
    /// crumb segments (kept split: minijinja would entity-escape a "/"
    /// inside an interpolated value).
    entries: Vec<(Vec<String>, &'a Article)>,
}

/// Write the whole AI surface. Returns the number of HTML pages written
/// (the `/print` pages), so the build's page count stays honest.
pub fn write_ai_surface(
    site: &Site,
    links: &LinkIndex,
    images: &ImageIndex,
    renderer: &Renderer,
    out: &Output,
    llms_full: bool,
) -> Result<usize> {
    let mut pages = 0;
    for product in &site.products {
        out.write(
            &mirror_path(out.dir(), &product.path),
            product_mirror(product).as_bytes(),
        )?;
        let base = vec![product.title.clone()];
        for item in product.items() {
            match item {
                ProductItem::Anthology { anthology } => {
                    pages += export_anthology(
                        site, links, images, renderer, out, llms_full, product, &base, anthology,
                    )?;
                }
                ProductItem::Topic { topic } => {
                    pages += write_bundle(
                        site,
                        links,
                        images,
                        renderer,
                        out,
                        product,
                        &topic_unit(&base, topic),
                        llms_full,
                    )?;
                }
                ProductItem::Book { book } => {
                    out.write(
                        &mirror_path(out.dir(), &book.path),
                        book_mirror(book).as_bytes(),
                    )?;
                    pages += write_bundle(
                        site,
                        links,
                        images,
                        renderer,
                        out,
                        product,
                        &book_unit(&base, book),
                        llms_full,
                    )?;
                }
            }
        }
        pages += write_bundle(
            site,
            links,
            images,
            renderer,
            out,
            product,
            &product_unit(product),
            llms_full,
        )?;
    }
    write_article_mirrors(site, links, images, out)?;
    out.write(&out.dir().join("llms.txt"), llms_txt(site).as_bytes())?;
    out.write(
        &out.dir().join("site.json"),
        serde_json::to_string_pretty(&site_json(site, links))?.as_bytes(),
    )?;
    out.write(&out.dir().join("robots.txt"), robots_txt(site).as_bytes())?;
    if site.config.url.is_some() {
        out.write(&out.dir().join("sitemap.xml"), sitemap_xml(site).as_bytes())?;
    }
    Ok(pages)
}

/// Mirror and bundle an anthology and everything beneath it — anthologies
/// nest, so this recurses with a growing crumb base.
#[allow(clippy::too_many_arguments)]
fn export_anthology(
    site: &Site,
    links: &LinkIndex,
    images: &ImageIndex,
    renderer: &Renderer,
    out: &Output,
    llms_full: bool,
    product: &Product,
    base: &[String],
    anthology: &Anthology,
) -> Result<usize> {
    out.write(
        &mirror_path(out.dir(), &anthology.path),
        anthology_mirror(anthology).as_bytes(),
    )?;
    let mut child_base = base.to_vec();
    child_base.push(anthology.title.clone());
    let mut pages = 0;
    for item in anthology.items() {
        match item {
            AnthologyItem::Topic { topic } => {
                pages += write_bundle(
                    site,
                    links,
                    images,
                    renderer,
                    out,
                    product,
                    &topic_unit(&child_base, topic),
                    llms_full,
                )?;
            }
            AnthologyItem::Book { book } => {
                out.write(
                    &mirror_path(out.dir(), &book.path),
                    book_mirror(book).as_bytes(),
                )?;
                pages += write_bundle(
                    site,
                    links,
                    images,
                    renderer,
                    out,
                    product,
                    &book_unit(&child_base, book),
                    llms_full,
                )?;
            }
            AnthologyItem::Anthology { anthology } => {
                pages += export_anthology(
                    site,
                    links,
                    images,
                    renderer,
                    out,
                    llms_full,
                    product,
                    &child_base,
                    anthology,
                )?;
            }
        }
    }
    pages += write_bundle(
        site,
        links,
        images,
        renderer,
        out,
        product,
        &anthology_unit(base, anthology),
        llms_full,
    )?;
    Ok(pages)
}

// ---- Units ----------------------------------------------------------------

fn product_unit(product: &Product) -> Unit<'_> {
    let mut entries = Vec::new();
    let base = vec![product.title.clone()];
    for item in product.items() {
        match item {
            ProductItem::Anthology { anthology } => {
                anthology_entries(&base, anthology, &mut entries)
            }
            ProductItem::Topic { topic } => topic_entries(&base, topic, &mut entries),
            ProductItem::Book { book } => book_entries(&base, book, &mut entries),
        }
    }
    Unit {
        title: product.title.clone(),
        description: Some(&product.description),
        path: &product.path,
        entries,
    }
}

fn anthology_unit<'a>(base: &[String], anthology: &'a Anthology) -> Unit<'a> {
    let mut entries = Vec::new();
    anthology_entries(base, anthology, &mut entries);
    Unit {
        title: anthology.title.clone(),
        description: Some(&anthology.description),
        path: &anthology.path,
        entries,
    }
}

fn topic_unit<'a>(base: &[String], topic: &'a Topic) -> Unit<'a> {
    let mut entries = Vec::new();
    topic_entries(base, topic, &mut entries);
    Unit {
        title: topic.title.clone(),
        description: None,
        path: &topic.path,
        entries,
    }
}

fn book_unit<'a>(base: &[String], book: &'a Book) -> Unit<'a> {
    let mut entries = Vec::new();
    book_entries(base, book, &mut entries);
    Unit {
        title: book.title.clone(),
        description: Some(&book.description),
        path: &book.path,
        entries,
    }
}

fn anthology_entries<'a>(
    base: &[String],
    anthology: &'a Anthology,
    entries: &mut Vec<(Vec<String>, &'a Article)>,
) {
    let mut child_base = base.to_vec();
    child_base.push(anthology.title.clone());
    for item in anthology.items() {
        match item {
            AnthologyItem::Topic { topic } => topic_entries(&child_base, topic, entries),
            AnthologyItem::Book { book } => book_entries(&child_base, book, entries),
            AnthologyItem::Anthology { anthology } => {
                anthology_entries(&child_base, anthology, entries)
            }
        }
    }
}

fn topic_entries<'a>(
    base: &[String],
    topic: &'a Topic,
    entries: &mut Vec<(Vec<String>, &'a Article)>,
) {
    let mut topic_base = base.to_vec();
    topic_base.push(topic.title.clone());
    for child in &topic.children {
        match child {
            TopicChild::Article { article } => entries.push((topic_base.clone(), article)),
            TopicChild::Folder { folder } => {
                for article in &folder.articles {
                    let mut crumbs = topic_base.clone();
                    crumbs.push(folder.title.clone());
                    entries.push((crumbs, article));
                }
            }
        }
    }
}

fn book_entries<'a>(
    base: &[String],
    book: &'a Book,
    entries: &mut Vec<(Vec<String>, &'a Article)>,
) {
    let name = book.short.as_deref().unwrap_or(&book.title);
    for (chapters, article) in book.articles() {
        let mut crumbs = base.to_vec();
        crumbs.push(name.to_string());
        crumbs.extend(chapters.iter().map(|chapter| chapter.title.clone()));
        entries.push((crumbs, article));
    }
}

// ---- Bundles ---------------------------------------------------------------

/// Write a unit's `/print` page and `/print.md` — plus, when `llms_full`
/// is set, an `llms-full.txt` copy of print.md (a byte copy: GitHub Pages
/// does not serve symlinks) for agents that probe for that name instead
/// of reading llms.txt. Empty units get no bundle. Returns the number of
/// HTML pages written (0 or 1).
#[allow(clippy::too_many_arguments)]
fn write_bundle(
    site: &Site,
    links: &LinkIndex,
    images: &ImageIndex,
    renderer: &Renderer,
    out: &Output,
    product: &Product,
    unit: &Unit,
    llms_full: bool,
) -> Result<usize> {
    if unit.entries.is_empty() {
        return Ok(0);
    }

    let markdown = bundle_markdown(links, images, unit);
    let md_url = format!("{}/print.md", unit.path);
    // Every page in this bundle, by the anchor that reaches it here.
    let anchors: HashMap<String, String> = unit
        .entries
        .iter()
        .map(|(_, article)| {
            (
                article
                    .original
                    .clone()
                    .unwrap_or_else(|| article.path.clone()),
                print_anchor(unit.path, article),
            )
        })
        .collect();
    out.write(
        &mirror_path(out.dir(), &format!("{}/print", unit.path)),
        markdown.as_bytes(),
    )?;
    if llms_full {
        out.write(
            &out.dir().join(format!(
                "{}/llms-full.txt",
                unit.path.trim_start_matches('/')
            )),
            markdown.as_bytes(),
        )?;
    }

    let mut sections = Vec::new();
    let mut has_mermaid = false;
    for (crumbs, article) in &unit.entries {
        let id = print_anchor(unit.path, article);
        // Dangling links were already reported (or rejected) when the
        // article's own page rendered, so this pass stays quiet.
        let rendered = markdown::render(
            &article.body,
            links,
            markdown::RenderOptions {
                allow_dangling: true,
                numbering: article.number.as_deref(),
                id_prefix: Some(&id),
                refs: None,
                print: Some(markdown::PrintScope {
                    anchors: &anchors,
                    base: site.config.url.as_deref(),
                }),
                images: Some(ImageScope {
                    index: images,
                    dir: &article.source_dir,
                }),
            },
        )?;
        has_mermaid |= rendered.has_mermaid;
        sections.push(PrintSection {
            id,
            number: article.number.clone(),
            appendix: article.appendix,
            title: article.title.clone(),
            crumbs: crumbs.clone(),
            html: rendered.html,
        });
    }
    let html = renderer.print(
        site,
        product,
        &unit.title,
        unit.description,
        unit.path,
        &md_url,
        &sections,
        has_mermaid,
    )?;
    out.write(
        &out.dir().join(format!(
            "{}/print/index.html",
            unit.path.trim_start_matches('/')
        )),
        html.as_bytes(),
    )?;
    Ok(1)
}

/// The whole unit as one markdown document, in reading order.
fn bundle_markdown(links: &LinkIndex, images: &ImageIndex, unit: &Unit) -> String {
    let mut out = format!("# {}\n", unit.title);
    if let Some(description) = unit.description {
        let _ = write!(out, "\n> {description}\n");
    }
    for (crumbs, article) in &unit.entries {
        let _ = write!(
            out,
            "\n---\n\n{}",
            article_markdown(links, images, &crumbs.join(" / "), article)
        );
    }
    out
}

/// One article as standalone markdown: title (numbered inside books),
/// crumbs for orientation, description, then the body with `~` links
/// resolved to the target pages' own mirrors.
fn article_markdown(
    links: &LinkIndex,
    images: &ImageIndex,
    crumbs: &str,
    article: &Article,
) -> String {
    let mut out = String::from("# ");
    if let Some(number) = &article.number {
        if article.appendix {
            let _ = write!(out, "Appendix ");
        }
        let _ = write!(out, "{number} ");
    }
    let _ = writeln!(out, "{}", article.title);
    let _ = write!(out, "\n_{crumbs}_\n");
    if let Some(description) = &article.description {
        let _ = write!(out, "\n> {description}\n");
    }
    let body = markdown::rewrite_source(
        &article.body,
        links,
        article.number.as_deref(),
        Some(ImageScope {
            index: images,
            dir: &article.source_dir,
        }),
    );
    let _ = write!(out, "\n{}", body.trim_end());
    out.push('\n');
    out
}

/// An article's `related:` references resolved to (path, title) — kept
/// quiet: anything broken was already reported (or rejected) when the
/// article's own page rendered, so this pass just drops failures.
fn related_pages(links: &LinkIndex, article: &Article) -> Vec<(String, String)> {
    article
        .related
        .iter()
        .filter_map(|reference| links.resolve_page(reference).ok())
        .collect()
}

// ---- Page mirrors ----------------------------------------------------------

fn write_article_mirrors(
    site: &Site,
    links: &LinkIndex,
    images: &ImageIndex,
    out: &Output,
) -> Result<()> {
    for product in &site.products {
        for (crumbs, article) in product_unit(product).entries {
            let mut markdown = article_markdown(links, images, &crumbs.join(" / "), article);
            // Standalone mirrors carry the Related Content list the page
            // shows, pointing at the targets' own mirrors. Bundles skip
            // it — a linear document doesn't want per-article footers.
            let related = related_pages(links, article);
            if !related.is_empty() {
                markdown.push_str("\nRelated content:\n\n");
                for (path, title) in &related {
                    let _ = writeln!(markdown, "- [{title}]({path}.md)");
                }
            }
            out.write(&mirror_path(out.dir(), &article.path), markdown.as_bytes())?;
        }
    }
    Ok(())
}

fn product_mirror(product: &Product) -> String {
    let mut out = format!("# {}\n\n> {}\n", product.title, product.description);
    let _ = write!(
        out,
        "\nAll of {} as one markdown file: [{path}/print.md]({path}/print.md)\n",
        product.title,
        path = product.path
    );
    for section in product.sections() {
        if let Some(title) = section.title {
            let _ = write!(out, "\n## {title}\n");
        }
        out.push('\n');
        for item in section.items {
            match item {
                ProductItem::Anthology { anthology } => {
                    let _ = writeln!(
                        out,
                        "- [{}]({}.md): {}",
                        anthology.title, anthology.path, anthology.description
                    );
                }
                ProductItem::Topic { topic } => {
                    let _ = writeln!(
                        out,
                        "- [{}]({}/print.md): {} articles",
                        topic.title,
                        topic.path,
                        topic.pages().count()
                    );
                }
                ProductItem::Book { book } => {
                    let _ = writeln!(
                        out,
                        "- [{}]({}.md): {}",
                        book.title, book.path, book.description
                    );
                }
            }
        }
    }
    out
}

fn anthology_mirror(anthology: &Anthology) -> String {
    let mut out = format!("# {}\n\n> {}\n", anthology.title, anthology.description);
    let _ = write!(
        out,
        "\nAll of {} as one markdown file: [{path}/print.md]({path}/print.md)\n",
        anthology.title,
        path = anthology.path
    );
    // Nested anthologies and books first, as an annotated list...
    let mut listed = false;
    for item in anthology.items() {
        let line = match item {
            AnthologyItem::Anthology { anthology } => Some(format!(
                "- [{}]({}.md): {}",
                anthology.title, anthology.path, anthology.description
            )),
            AnthologyItem::Book { book } => Some(format!(
                "- [{}]({}.md): {}",
                book.title, book.path, book.description
            )),
            AnthologyItem::Topic { .. } => None,
        };
        if let Some(line) = line {
            if !listed {
                out.push('\n');
                listed = true;
            }
            let _ = writeln!(out, "{line}");
        }
    }
    // ...then each direct topic with its articles.
    for topic in anthology.topics() {
        let _ = write!(out, "\n## {}\n\n", topic.title);
        for article in topic.pages() {
            let _ = writeln!(
                out,
                "- [{}]({}.md): {}",
                article.title,
                article.path,
                article.description.as_deref().unwrap_or_default()
            );
        }
    }
    out
}

fn book_mirror(book: &Book) -> String {
    let mut out = String::from("# ");
    match &book.short {
        Some(short) => {
            let _ = writeln!(out, "{} ({short})", book.title);
        }
        None => {
            let _ = writeln!(out, "{}", book.title);
        }
    }
    let _ = write!(out, "\n> {}\n", book.description);
    let _ = write!(
        out,
        "\nThe whole book as one markdown file: [{path}/print.md]({path}/print.md)\n\
         \nContents:\n\n",
        path = book.path
    );
    fn contents(out: &mut String, children: &[BookChild], depth: usize) {
        for child in children {
            let indent = "    ".repeat(depth);
            match child {
                BookChild::Article { article } => {
                    let number = article.number.as_deref().unwrap_or_default();
                    let label = if article.appendix { "Appendix " } else { "" };
                    let _ = writeln!(
                        out,
                        "{indent}- {label}{number} [{}]({}.md)",
                        article.title, article.path
                    );
                }
                BookChild::Chapter { chapter } => {
                    let label = if chapter.appendix { "Appendix " } else { "" };
                    let _ = writeln!(
                        out,
                        "{indent}- {label}{} [{}]({}.md)",
                        chapter.number, chapter.title, chapter.entry
                    );
                    contents(out, &chapter.children, depth + 1);
                }
            }
        }
    }
    contents(&mut out, &book.children, 0);
    out
}

// ---- Site-wide files -------------------------------------------------------

/// The llms.txt convention (llmstxt.org): a curated markdown map of the
/// site for AI consumers, linking the markdown mirrors and bundles.
fn llms_txt(site: &Site) -> String {
    fn anthology_lines(out: &mut String, anthology: &Anthology) {
        let _ = writeln!(
            out,
            "- [{}]({}/print.md): {}",
            anthology.title, anthology.path, anthology.description
        );
        for item in anthology.items() {
            match item {
                AnthologyItem::Book { book } => {
                    let _ = writeln!(
                        out,
                        "- [{}]({}/print.md): {}",
                        book.title, book.path, book.description
                    );
                }
                AnthologyItem::Anthology { anthology } => anthology_lines(out, anthology),
                AnthologyItem::Topic { .. } => {}
            }
        }
    }

    let mut out = format!(
        "# {}\n\n> {}\n",
        site.config.sitename, site.config.description
    );
    out.push_str(
        "\nEvery page on this site is also available as markdown at the same URL \
         with `.md` appended. Each section below links whole units as single \
         markdown files. The full machine-readable structure is at /site.json.\n",
    );
    for product in &site.products {
        let _ = write!(out, "\n## {}\n\n", product.title);
        let _ = writeln!(
            out,
            "- [{} overview]({}.md): {}",
            product.title, product.path, product.description
        );
        let _ = writeln!(
            out,
            "- [All {} docs in one file]({}/print.md)",
            product.title, product.path
        );
        for item in product.items() {
            match item {
                ProductItem::Anthology { anthology } => anthology_lines(&mut out, anthology),
                ProductItem::Topic { topic } => {
                    let count = topic.pages().count();
                    if count > 0 {
                        let _ = writeln!(
                            out,
                            "- [{}]({}/print.md): {count} articles",
                            topic.title, topic.path
                        );
                    }
                }
                ProductItem::Book { book } => {
                    let _ = writeln!(
                        out,
                        "- [{}]({}/print.md): {}",
                        book.title, book.path, book.description
                    );
                }
            }
        }
    }
    out
}

/// The whole model as JSON, for tools that want structure, not prose.
fn site_json(site: &Site, links: &LinkIndex) -> serde_json::Value {
    fn article_json(article: &Article, links: &LinkIndex) -> serde_json::Value {
        let mut value = json!({
            "slug": article.slug,
            "path": article.path,
            "md": format!("{}.md", article.path),
            "title": article.title,
            "type": article.kind,
            "description": article.description,
        });
        if let Some(number) = &article.number {
            value["number"] = json!(number);
        }
        if article.appendix {
            value["appendix"] = json!(true);
        }
        if let Some(original) = &article.original {
            value["original"] = json!(original);
        }
        let related: Vec<String> = related_pages(links, article)
            .into_iter()
            .map(|(path, _)| path)
            .collect();
        if !related.is_empty() {
            value["related"] = json!(related);
        }
        value
    }
    fn topic_json(topic: &Topic, links: &LinkIndex) -> serde_json::Value {
        json!({
            "kind": "topic",
            "slug": topic.slug,
            "path": topic.path,
            "print": format!("{}/print.md", topic.path),
            "title": topic.title,
            "entry": topic.entry(),
            "children": topic.children.iter().map(|child| match child {
                TopicChild::Article { article } => article_json(article, links),
                TopicChild::Folder { folder } => json!({
                    "kind": "folder",
                    "slug": folder.slug,
                    "path": folder.path,
                    "title": folder.title,
                    "articles": folder.articles.iter()
                        .map(|article| article_json(article, links))
                        .collect::<Vec<_>>(),
                }),
            }).collect::<Vec<_>>(),
        })
    }
    fn book_children_json(children: &[BookChild], links: &LinkIndex) -> Vec<serde_json::Value> {
        children
            .iter()
            .map(|child| match child {
                BookChild::Article { article } => article_json(article, links),
                BookChild::Chapter { chapter } => {
                    let mut value = json!({
                        "kind": "chapter",
                        "slug": chapter.slug,
                        "number": chapter.number,
                        "title": chapter.title,
                        "entry": chapter.entry,
                        "children": book_children_json(&chapter.children, links),
                    });
                    if chapter.appendix {
                        value["appendix"] = json!(true);
                    }
                    value
                }
            })
            .collect()
    }
    fn book_json(book: &Book, links: &LinkIndex) -> serde_json::Value {
        json!({
            "kind": "book",
            "slug": book.slug,
            "path": book.path,
            "md": format!("{}.md", book.path),
            "print": format!("{}/print.md", book.path),
            "title": book.title,
            "short": book.short,
            "description": book.description,
            "children": book_children_json(&book.children, links),
        })
    }
    fn anthology_json(anthology: &Anthology, links: &LinkIndex) -> serde_json::Value {
        json!({
            "kind": "anthology",
            "slug": anthology.slug,
            "path": anthology.path,
            "md": format!("{}.md", anthology.path),
            "print": format!("{}/print.md", anthology.path),
            "title": anthology.title,
            "description": anthology.description,
            "items": anthology.items().map(|item| match item {
                AnthologyItem::Topic { topic } => topic_json(topic, links),
                AnthologyItem::Book { book } => book_json(book, links),
                AnthologyItem::Anthology { anthology } => anthology_json(anthology, links),
            }).collect::<Vec<_>>(),
        })
    }

    json!({
        "sitename": site.config.sitename,
        "title": site.config.title,
        "description": site.config.description,
        "url": site.config.url,
        "llms": "/llms.txt",
        "products": site.products.iter().map(|product| json!({
            "slug": product.slug,
            "path": product.path,
            "md": format!("{}.md", product.path),
            "print": format!("{}/print.md", product.path),
            "title": product.title,
            "monogram": product.monogram,
            "color": product.color,
            "description": product.description,
            "items": product.items().map(|item| match item {
                ProductItem::Anthology { anthology } => anthology_json(anthology, links),
                ProductItem::Topic { topic } => topic_json(topic, links),
                ProductItem::Book { book } => book_json(book, links),
            }).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

fn robots_txt(site: &Site) -> String {
    let mut out = String::from("User-agent: *\nAllow: /\n");
    if let Some(url) = &site.config.url {
        let _ = write!(out, "\nSitemap: {url}/sitemap.xml\n");
    }
    out
}

/// Every HTML page's URL path with its last-updated date, for the
/// sitemap ("" is the front page). Alias pages stay out: their canonical
/// is the original.
fn page_paths(site: &Site) -> Vec<(String, Option<String>)> {
    fn topic_paths(paths: &mut Vec<(String, Option<String>)>, topic: &Topic) {
        if topic.pages().next().is_some() {
            paths.push((format!("{}/print", topic.path), topic.updated.clone()));
        }
        for article in topic.pages() {
            if article.original.is_none() {
                paths.push((article.path.clone(), article.updated.clone()));
            }
        }
    }
    fn book_paths(paths: &mut Vec<(String, Option<String>)>, book: &Book) {
        paths.push((book.path.clone(), book.updated.clone()));
        if !book.articles().is_empty() {
            paths.push((format!("{}/print", book.path), book.updated.clone()));
        }
        for (_, article) in book.articles() {
            if article.original.is_none() {
                paths.push((article.path.clone(), article.updated.clone()));
            }
        }
    }
    fn anthology_paths(paths: &mut Vec<(String, Option<String>)>, anthology: &Anthology) {
        paths.push((anthology.path.clone(), anthology.updated.clone()));
        if !anthology_unit(&[], anthology).entries.is_empty() {
            paths.push((
                format!("{}/print", anthology.path),
                anthology.updated.clone(),
            ));
        }
        for item in anthology.items() {
            match item {
                AnthologyItem::Topic { topic } => topic_paths(paths, topic),
                AnthologyItem::Book { book } => book_paths(paths, book),
                AnthologyItem::Anthology { anthology } => anthology_paths(paths, anthology),
            }
        }
    }

    let site_updated = site
        .products
        .iter()
        .fold(None, |so_far: Option<String>, product| {
            match (so_far, product.updated.clone()) {
                (Some(a), Some(b)) => Some(if a >= b { a } else { b }),
                (Some(only), None) | (None, Some(only)) => Some(only),
                (None, None) => None,
            }
        });
    let mut paths = vec![(String::new(), site_updated)];
    for product in &site.products {
        paths.push((product.path.clone(), product.updated.clone()));
        if !product_unit(product).entries.is_empty() {
            paths.push((format!("{}/print", product.path), product.updated.clone()));
        }
        for item in product.items() {
            match item {
                ProductItem::Anthology { anthology } => anthology_paths(&mut paths, anthology),
                ProductItem::Topic { topic } => topic_paths(&mut paths, topic),
                ProductItem::Book { book } => book_paths(&mut paths, book),
            }
        }
    }
    paths
}

/// Absolute URLs are mandatory in sitemaps, so this is only written when
/// the site config carries a base `url`.
fn sitemap_xml(site: &Site) -> String {
    let base = site.config.url.as_deref().expect("caller checked");
    let mut out = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );
    for (path, updated) in page_paths(site) {
        match updated {
            Some(updated) => {
                let _ = writeln!(
                    out,
                    "  <url><loc>{base}{path}/</loc><lastmod>{updated}</lastmod></url>"
                );
            }
            None => {
                let _ = writeln!(out, "  <url><loc>{base}{path}/</loc></url>");
            }
        }
    }
    out.push_str("</urlset>\n");
    out
}

/// A page's anchor id within its unit's `/print` bundle: its path below
/// the unit, flattened. Shared so the bundle and the links pointing into
/// it can never disagree.
pub(crate) fn print_anchor(unit_path: &str, article: &Article) -> String {
    article
        .path
        .strip_prefix(&format!("{unit_path}/"))
        .unwrap_or(&article.slug)
        .replace('/', "-")
}

/// Where a page's markdown mirror lands: the page URL with `.md` appended
/// ("/alpha/acorn" → "<out>/alpha/acorn.md").
fn mirror_path(out: &Path, url_path: &str) -> std::path::PathBuf {
    out.join(format!("{}.md", url_path.trim_start_matches('/')))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Renderer;
    use std::fs;

    fn build_surface(root: &Path, llms_full: bool) -> (std::path::PathBuf, usize) {
        let out = root.join("dist");
        let site = Site::load(root, &out).unwrap();
        let links = LinkIndex::new(&site);
        let images = ImageIndex::new(&site.images);
        let renderer = Renderer::new(false).unwrap();
        let output = Output::new(&out);
        let pages =
            write_ai_surface(&site, &links, &images, &renderer, &output, llms_full).unwrap();
        (out, pages)
    }

    fn fixture_surface() -> (tempfile::TempDir, std::path::PathBuf, usize) {
        let dir = tempfile::tempdir().unwrap();
        crate::site::write_fixture(dir.path());
        let (out, pages) = build_surface(dir.path(), false);
        (dir, out, pages)
    }

    #[test]
    fn every_page_gets_a_markdown_mirror() {
        let (_dir, out, _) = fixture_surface();

        let article = fs::read_to_string(out.join("alpha/acorn/wide/a1.md")).unwrap();
        assert!(article.starts_with("# Article a1\n"));
        assert!(article.contains("_Alpha / Acorn Docs / Wide Topic_"));
        assert!(article.contains("> about a1"));
        assert!(article.contains("Body of a1."));
        // The mirror carries the page's Related Content list, pointing
        // at the targets' own mirrors.
        assert!(article.contains("Related content:"));
        assert!(article.contains("- [Article b1](/alpha/acorn/narrow/b1.md)"));
        assert!(article.contains("- [Alpha Manual](/alpha/manual.md)"));

        // Book pages carry their section numbers, in title and headings,
        // and no description (book frontmatter is title-only).
        let tuning = fs::read_to_string(out.join("alpha/manual/setup/advanced/tuning.md")).unwrap();
        assert!(tuning.starts_with("# 2.2.1 Article tuning\n"));
        assert!(tuning.contains("## 2.2.1.1 First section of tuning"));
        assert!(!tuning.contains("> about"));
        // Appendix pages get the full label; their headings stay short.
        let glossary = fs::read_to_string(out.join("alpha/manual/glossary.md")).unwrap();
        assert!(glossary.starts_with("# Appendix A Article glossary\n"));
        assert!(glossary.contains("## A.1 First section of glossary"));

        // Articles under nested anthologies carry the whole chain.
        let deep = fs::read_to_string(out.join("alpha/acorn/inner/deep/d1.md")).unwrap();
        assert!(deep.contains("_Alpha / Acorn Docs / Inner Docs / Deep_"));

        // Image destinations are rewritten to their published URLs —
        // mirrors and bundles serve the body away from its own folder.
        let pic = fs::read_to_string(out.join("alpha/loose/pic.md")).unwrap();
        assert!(pic.contains("![Wiring overview](/alpha/loose/wiring.png \"The wiring\")"));
        let install = fs::read_to_string(out.join("alpha/manual/setup/install.md")).unwrap();
        assert!(install.contains("![Layout again](/alpha/manual/layout.png)"));

        // Container pages mirror as annotated listings.
        let product = fs::read_to_string(out.join("alpha.md")).unwrap();
        assert!(product.starts_with("# Alpha\n"));
        assert!(product.contains("[/alpha/print.md](/alpha/print.md)"));
        assert!(product.contains("- [Acorn Docs](/alpha/acorn.md): anthology description"));
        assert!(product.contains("- [Loose](/alpha/loose/print.md): 5 articles"));
        assert!(product.contains("## Tools"), "shelf sections survive");

        let anthology = fs::read_to_string(out.join("alpha/acorn.md")).unwrap();
        assert!(anthology.contains("## Wide Topic"));
        assert!(anthology.contains("- [Article a1](/alpha/acorn/wide/a1.md): about a1"));
        assert!(anthology.contains("- [Acorn Spec](/alpha/acorn/spec.md): spec description"));
        assert!(anthology.contains("- [Inner Docs](/alpha/acorn/inner.md): inner description"));

        let book = fs::read_to_string(out.join("alpha/manual.md")).unwrap();
        assert!(book.starts_with("# Alpha Manual (AM)\n"));
        assert!(book.contains("- 2 [Setup](/alpha/manual/setup/install.md)"));
        assert!(book.contains("- Appendix A [Article glossary](/alpha/manual/glossary.md)"));
        assert!(book.contains("- Appendix B [History](/alpha/manual/history/old.md)"));
        assert!(book.contains("    - 2.1 [Article install](/alpha/manual/setup/install.md)"));
        assert!(
            book.contains(
                "        - 2.2.1 [Article tuning](/alpha/manual/setup/advanced/tuning.md)"
            )
        );
    }

    #[test]
    fn units_get_print_bundles_and_empty_units_do_not() {
        let (_dir, out, pages) = fixture_surface();
        // alpha product + acorn + inner + 6 topics + 3 books; beta and the
        // empty anthologies get nothing.
        assert_eq!(pages, 12);

        let bundle = fs::read_to_string(out.join("alpha/acorn/wide/print.md")).unwrap();
        assert!(bundle.starts_with("# Wide Topic\n"));
        assert!(bundle.contains("# Article a1"));
        assert!(bundle.contains("# Article a5"));
        assert!(
            !bundle.contains("Related content:"),
            "linear bundles skip per-article related footers"
        );
        assert!(
            !out.join("alpha/acorn/wide/llms-full.txt").exists(),
            "llms-full.txt is opt-in"
        );

        // The acorn bundle spans its book and nested anthology too.
        let acorn = fs::read_to_string(out.join("alpha/acorn/print.md")).unwrap();
        assert!(
            acorn.contains("# 1 Article rules"),
            "book articles keep numbers"
        );
        assert!(acorn.contains("# Article d1"));

        // The print page namespaces heading anchors per article and stays
        // out of the search index.
        let html = fs::read_to_string(out.join("alpha/print/index.html")).unwrap();
        assert!(html.contains("id=\"acorn-wide-a1--first-section-of-a1\""));
        assert!(html.contains(">Alpha</h1>"));
        // Print sections label appendices in full too.
        let manual = fs::read_to_string(out.join("alpha/manual/print/index.html")).unwrap();
        assert!(
            manual.contains("<span class=\"heading-number\">Appendix A</span> Article glossary")
        );
        let manual_md = fs::read_to_string(out.join("alpha/manual/print.md")).unwrap();
        assert!(manual_md.contains("# Appendix A Article glossary"));
        assert!(html.contains("Alpha / Acorn Docs / Wide Topic"));
        assert!(!html.contains("data-pagefind-body"));

        assert!(!out.join("beta/print.md").exists());
        assert!(!out.join("alpha/zulu/print.md").exists());
    }

    #[test]
    fn llms_full_copies_are_opt_in() {
        let dir = tempfile::tempdir().unwrap();
        crate::site::write_fixture(dir.path());
        let (out, _) = build_surface(dir.path(), true);
        let bundle = fs::read_to_string(out.join("alpha/acorn/wide/print.md")).unwrap();
        let full = fs::read_to_string(out.join("alpha/acorn/wide/llms-full.txt")).unwrap();
        assert_eq!(bundle, full, "llms-full.txt is a copy of print.md");
        // llms.txt keeps pointing at the canonical print.md either way.
        let llms = fs::read_to_string(out.join("llms.txt")).unwrap();
        assert!(!llms.contains("llms-full"));
    }

    #[test]
    fn llms_txt_maps_the_site_for_ai() {
        let (_dir, out, _) = fixture_surface();
        let llms = fs::read_to_string(out.join("llms.txt")).unwrap();
        assert!(llms.starts_with("# Test Learn\n\n> A fixture site.\n"));
        assert!(llms.contains("with `.md` appended"));
        assert!(llms.contains("## Alpha"));
        assert!(llms.contains("- [Alpha overview](/alpha.md): a description"));
        assert!(llms.contains("- [All Alpha docs in one file](/alpha/print.md)"));
        assert!(llms.contains("- [Acorn Docs](/alpha/acorn/print.md): anthology description"));
        assert!(llms.contains("- [Loose](/alpha/loose/print.md): 5 articles"));
        assert!(llms.contains("- [Alpha Manual](/alpha/manual/print.md): book description"));
        // Books and anthologies nested inside anthologies are listed too.
        assert!(llms.contains("- [Acorn Spec](/alpha/acorn/spec/print.md): spec description"));
        assert!(llms.contains("- [Inner Docs](/alpha/acorn/inner/print.md): inner description"));
    }

    #[test]
    fn site_json_serialises_the_whole_model() {
        let (_dir, out, _) = fixture_surface();
        let json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(out.join("site.json")).unwrap()).unwrap();
        assert_eq!(json["sitename"], "Test Learn");
        assert_eq!(json["url"], serde_json::Value::Null);
        let alpha = &json["products"][1];
        assert_eq!(alpha["slug"], "alpha");
        assert_eq!(alpha["print"], "/alpha/print.md");
        let acorn = &alpha["items"][0];
        assert_eq!(acorn["kind"], "anthology");
        assert_eq!(
            acorn["items"][0]["children"][0]["md"],
            "/alpha/acorn/wide/a1.md"
        );
        // Books nest under anthologies; anthologies nest recursively.
        let kinds: Vec<_> = acorn["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["kind"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(kinds, ["topic", "topic", "topic", "book", "anthology"]);
        assert_eq!(acorn["items"][4]["items"][0]["kind"], "topic");
        // Book articles have no type/description.
        let manual = alpha["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|item| item["slug"] == "manual")
            .unwrap();
        assert_eq!(manual["children"][1]["kind"], "chapter");
        assert_eq!(manual["children"][1]["number"], "2");
        assert_eq!(manual["children"][0]["type"], serde_json::Value::Null);
        // `related:` references come out resolved to page paths.
        let a1 = &acorn["items"][0]["children"][0];
        assert_eq!(
            a1["related"],
            serde_json::json!(["/alpha/acorn/narrow/b1", "/alpha/manual"])
        );
        assert_eq!(
            manual["children"][0]["related"],
            serde_json::Value::Null,
            "articles without related get no key"
        );
    }

    #[test]
    fn print_bundles_link_inside_themselves_and_out_absolutely() {
        let dir = tempfile::tempdir().unwrap();
        crate::site::write_fixture(dir.path());
        let config = fs::read_to_string(dir.path().join("trail.toml")).unwrap();
        fs::write(
            dir.path().join("trail.toml"),
            format!("url = \"https://docs.example\"\n{config}"),
        )
        .unwrap();
        let (out, _) = build_surface(dir.path(), false);

        let print = fs::read_to_string(out.join("alpha/manual/print/index.html")).unwrap();
        // A page in this bundle becomes an in-document anchor — and a
        // heading in it keeps that page's id prefix.
        assert!(print.contains("href=\"#setup-install--first-section-of-install\""));
        // A page outside it goes absolute, so the link survives print,
        // PDF, and reading the file away from the site.
        assert!(print.contains("href=\"https://docs.example/alpha/acorn/narrow/b1\""));
        // Ordinary pages are unaffected: this rewriting is print-only
        // (their own links are covered in the render tests).
        assert!(!print.contains("href=\"/alpha/manual/setup/install"));
    }

    #[test]
    fn sitemap_needs_a_base_url_and_robots_is_always_there() {
        let (dir, out, _) = fixture_surface();
        let robots = fs::read_to_string(out.join("robots.txt")).unwrap();
        assert!(robots.contains("Allow: /"));
        assert!(!robots.contains("Sitemap:"));
        assert!(!out.join("sitemap.xml").exists());

        let config = fs::read_to_string(dir.path().join("trail.toml")).unwrap();
        fs::write(
            dir.path().join("trail.toml"),
            format!("url = \"https://docs.example/\"\n{config}"),
        )
        .unwrap();
        let (out, _) = build_surface(dir.path(), false);
        let sitemap = fs::read_to_string(out.join("sitemap.xml")).unwrap();
        assert!(sitemap.contains("<loc>https://docs.example/</loc>"));
        assert!(sitemap.contains("<loc>https://docs.example/alpha/acorn/wide/a1/</loc>"));
        assert!(sitemap.contains("<loc>https://docs.example/alpha/print/</loc>"));
        assert!(sitemap.contains("<loc>https://docs.example/alpha/acorn/inner/deep/d1/</loc>"));
        // Dated pages carry lastmod, rolled up for containers.
        assert!(sitemap.contains(
            "<loc>https://docs.example/alpha/loose/x2/</loc><lastmod>2026-03-15</lastmod>"
        ));
        assert!(sitemap.contains(
            "<loc>https://docs.example/alpha/manual/</loc><lastmod>2026-05-01</lastmod>"
        ));
        // Undated pages simply omit it.
        assert!(sitemap.contains("<loc>https://docs.example/alpha/acorn/wide/a1/</loc></url>"));
        // Alias pages stay out — their canonical is the original.
        assert!(!sitemap.contains("/alpha/loose/alias/"));
        assert!(!sitemap.contains("/alpha/acorn/narrow/extra/linked/"));
        let robots = fs::read_to_string(out.join("robots.txt")).unwrap();
        assert!(robots.contains("Sitemap: https://docs.example/sitemap.xml"));
    }
}
