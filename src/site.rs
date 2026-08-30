use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

use crate::images::{IMAGE_EXTENSIONS, ImageAsset};

/// The site model: everything the renderer needs, loaded and validated.
#[derive(Debug)]
pub struct Site {
    pub config: SiteConfig,
    /// All products in display order: featured first (in `featured` order),
    /// then the rest sorted by title.
    pub products: Vec<Product>,
    /// Every image file found alongside articles, wherever it sits in the
    /// tree — a flat registry (relative destinations can reach across
    /// containers), kept off the presentation model.
    pub images: Vec<ImageAsset>,
    /// The configured custom stylesheet resolved to its source file,
    /// copied to /assets/custom.css at build time.
    pub custom_css: Option<PathBuf>,
    /// The configured favicon resolved to its source file.
    pub favicon: Option<PathBuf>,
    /// The head_html snippet's contents, injected into every head.
    pub head_html: Option<String>,
    /// The site root directory, for deriving source-relative paths
    /// (edit links).
    pub root: PathBuf,
}

/// The root `trail.toml`. Unknown keys are load errors.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SiteConfig {
    pub sitename: String,
    pub title: String,
    pub description: String,
    /// The site's public base URL ("https://learn.peios.org"), used for
    /// the absolute links sitemap.xml requires. Optional: without it the
    /// sitemap is skipped and everything else uses root-relative paths.
    #[serde(default)]
    pub url: Option<String>,
    /// Slugs of the products shown as cards on the front page, in order.
    #[serde(default)]
    pub featured: Vec<String>,
    pub footer: String,
    /// The site accent color ("#635bff"): links, focus rings, the
    /// wordmark highlight, the hero glow. Its dark-mode variant is
    /// derived automatically unless `accent_dark` overrides it.
    #[serde(default)]
    pub accent: Option<String>,
    #[serde(default)]
    pub accent_dark: Option<String>,
    /// An extra stylesheet, path relative to the site root, shipped at
    /// /assets/custom.css and loaded after the built-in one — the
    /// theming escape hatch. Overriding the `--*` tokens goes furthest;
    /// anything beyond is ordinary CSS.
    #[serde(default)]
    pub custom_css: Option<String>,
    /// Path (relative to the site root) of the favicon file, shipped at
    /// the output root under its own name and linked from every page.
    #[serde(default)]
    pub favicon: Option<String>,
    /// Path of an HTML snippet file injected verbatim at the end of
    /// every page's head — analytics, fonts, extra meta tags.
    #[serde(default)]
    pub head_html: Option<String>,
    /// Where "Edit this page" links point: a URL template whose "{path}"
    /// placeholder becomes the article's source path relative to the
    /// site root ("https://github.com/org/repo/edit/main/{path}").
    #[serde(default)]
    pub edit_url: Option<String>,
    /// Links shown in the site header next to the products menu.
    #[serde(default)]
    pub nav: Vec<NavItem>,
    /// Root entries trail neither builds nor understands, copied into
    /// the output verbatim: a CNAME, a `.well-known/` directory, a
    /// verification file. Naming one is also what makes it legal to
    /// keep in the site root, exactly as `favicon` and friends do.
    /// They are copied last, so a passthrough deliberately named after
    /// something trail generates (`robots.txt`) replaces it.
    #[serde(default)]
    pub passthrough: Vec<String>,
    /// Per-product tinting (cards, tiles, sidebar highlights take each
    /// product's color). Off, everything tints from the site accent.
    #[serde(default = "default_true")]
    pub product_theming: bool,
    #[serde(default = "default_true")]
    pub built_by_trail: bool,
}

/// One header navigation link. The url may be external, or a `~` page
/// reference in the body-link grammar — resolved strictly at load,
/// because site chrome must never dangle.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NavItem {
    pub label: String,
    pub url: String,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
pub struct Product {
    pub slug: String,
    /// Site-absolute URL path, e.g. "/peios". Built from validated slugs.
    pub path: String,
    pub title: String,
    pub monogram: String,
    pub color: String,
    pub description: String,
    /// Everything under this product, added up; see `Article`.
    pub reading_minutes: u32,
    /// The most recent `updated:` date anywhere under this product.
    pub updated: Option<String>,
    /// Phrases from trail.toml (`inline_ref`) that auto-link to this
    /// product's page wherever prose states them.
    #[serde(skip)]
    pub inline_refs: Vec<String>,
    /// Direct children (anthologies, books, bare topics, shelves) in
    /// `NN--` order. Templates receive display `sections()` instead of this.
    #[serde(skip)]
    pub children: Vec<ProductChild>,
}

impl Product {
    /// The product's anthologies, books and topics in display order, with
    /// shelf contents flattened in place (a shelf is presentation only).
    pub fn items(&self) -> impl Iterator<Item = &ProductItem> {
        self.children.iter().flat_map(|child| match child {
            ProductChild::Item(item) => std::slice::from_ref(item).iter(),
            ProductChild::Shelf(shelf) => shelf.items.iter(),
        })
    }

    /// Display sections for the product page: runs of loose items coalesce
    /// into untitled sections; each shelf is a titled one.
    pub fn sections(&self) -> Vec<Section<'_, ProductItem>> {
        let mut sections = Vec::new();
        let mut loose: Vec<&ProductItem> = Vec::new();
        for child in &self.children {
            match child {
                ProductChild::Item(item) => loose.push(item),
                ProductChild::Shelf(shelf) => {
                    if !loose.is_empty() {
                        sections.push(Section {
                            title: None,
                            items: std::mem::take(&mut loose),
                        });
                    }
                    sections.push(Section {
                        title: Some(&shelf.title),
                        items: shelf.items.iter().collect(),
                    });
                }
            }
        }
        if !loose.is_empty() {
            sections.push(Section {
                title: None,
                items: loose,
            });
        }
        sections
    }

    #[cfg(test)]
    pub(crate) fn anthologies(&self) -> impl Iterator<Item = &Anthology> {
        self.items().filter_map(|item| match item {
            ProductItem::Anthology { anthology } => Some(anthology),
            _ => None,
        })
    }

    #[cfg(test)]
    pub(crate) fn books(&self) -> impl Iterator<Item = &Book> {
        self.items().filter_map(|item| match item {
            ProductItem::Book { book } => Some(book),
            _ => None,
        })
    }
}

/// A direct child of a product: a page-bearing item, or a shelf grouping
/// items. The nesting rule "a shelf never contains a shelf" is encoded
/// here: shelf items are `ProductItem`, which has no shelf variant.
#[derive(Debug)]
pub enum ProductChild {
    Item(ProductItem),
    Shelf(Shelf<ProductItem>),
}

impl ProductChild {
    fn order(&self) -> u32 {
        match self {
            ProductChild::Item(item) => item.order(),
            ProductChild::Shelf(shelf) => shelf.order,
        }
    }

    fn slug(&self) -> &str {
        match self {
            ProductChild::Item(item) => item.slug(),
            ProductChild::Shelf(shelf) => &shelf.slug,
        }
    }
}

/// A page-bearing product-level grouping. Serialized with a `kind` tag so
/// templates can dispatch on it.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ProductItem {
    Anthology {
        #[serde(flatten)]
        anthology: Anthology,
    },
    Topic {
        #[serde(flatten)]
        topic: Topic,
    },
    Book {
        #[serde(flatten)]
        book: Book,
    },
}

impl ProductItem {
    pub fn order(&self) -> u32 {
        match self {
            ProductItem::Anthology { anthology } => anthology.order,
            ProductItem::Topic { topic } => topic.order,
            ProductItem::Book { book } => book.order,
        }
    }

    pub fn slug(&self) -> &str {
        match self {
            ProductItem::Anthology { anthology } => &anthology.slug,
            ProductItem::Topic { topic } => &topic.slug,
            ProductItem::Book { book } => &book.slug,
        }
    }
}

/// A presentational grouping: a titled subsection of its parent's page.
/// It has no page and no URL segment of its own — its items live in the
/// parent's URL space, which is why slug uniqueness is checked flattened.
#[derive(Debug)]
pub struct Shelf<T> {
    pub slug: String,
    pub order: u32,
    /// Derived from the slug unless trail.toml overrides it.
    pub title: String,
    /// Items in the shelf's own `NN--` order.
    pub items: Vec<T>,
}

/// One display section of a product or anthology page.
#[derive(Debug, Serialize)]
pub struct Section<'a, T> {
    /// None for a run of loose items; the shelf title otherwise.
    pub title: Option<&'a str>,
    pub items: Vec<&'a T>,
}

/// A product's `trail.toml`. Unknown keys are load errors.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductConfig {
    title: String,
    monogram: String,
    color: String,
    description: String,
    #[serde(default)]
    inline_ref: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct Anthology {
    pub slug: String,
    /// Site-absolute URL path, e.g. "/peios/security-fundamentals".
    pub path: String,
    pub order: u32,
    pub title: String,
    pub description: String,
    /// Everything under this anthology, added up; see `Article`.
    pub reading_minutes: u32,
    /// The most recent `updated:` date anywhere under this anthology.
    pub updated: Option<String>,
    /// Phrases from trail.toml (`inline_ref`) that auto-link to this
    /// anthology's page wherever prose states them.
    #[serde(skip)]
    pub inline_refs: Vec<String>,
    /// Direct children (topics, shelves) in `NN--` order.
    /// Templates receive display `sections()` instead of this.
    #[serde(skip)]
    pub children: Vec<AnthologyChild>,
}

impl Anthology {
    /// The anthology's topics and books in display order, with shelf
    /// contents flattened in place (a shelf is presentation only).
    pub fn items(&self) -> impl Iterator<Item = &AnthologyItem> {
        self.children.iter().flat_map(|child| match child {
            AnthologyChild::Item(item) => std::slice::from_ref(item).iter(),
            AnthologyChild::Shelf(shelf) => shelf.items.iter(),
        })
    }

    /// Just the direct topics, for callers that don't care about books
    /// or nested anthologies.
    pub fn topics(&self) -> impl Iterator<Item = &Topic> {
        self.items().filter_map(|item| match item {
            AnthologyItem::Topic { topic } => Some(topic),
            _ => None,
        })
    }

    /// Display sections for the anthology page; see `Product::sections`.
    pub fn sections(&self) -> Vec<Section<'_, AnthologyItem>> {
        let mut sections = Vec::new();
        let mut loose: Vec<&AnthologyItem> = Vec::new();
        for child in &self.children {
            match child {
                AnthologyChild::Item(item) => loose.push(item),
                AnthologyChild::Shelf(shelf) => {
                    if !loose.is_empty() {
                        sections.push(Section {
                            title: None,
                            items: std::mem::take(&mut loose),
                        });
                    }
                    sections.push(Section {
                        title: Some(&shelf.title),
                        items: shelf.items.iter().collect(),
                    });
                }
            }
        }
        if !loose.is_empty() {
            sections.push(Section {
                title: None,
                items: loose,
            });
        }
        sections
    }
}

/// A direct child of an anthology: a page-bearing item, or a shelf
/// grouping items (never another shelf — see `ProductChild`).
#[derive(Debug)]
pub enum AnthologyChild {
    Item(AnthologyItem),
    Shelf(Shelf<AnthologyItem>),
}

impl AnthologyChild {
    fn order(&self) -> u32 {
        match self {
            AnthologyChild::Item(item) => item.order(),
            AnthologyChild::Shelf(shelf) => shelf.order,
        }
    }

    fn slug(&self) -> &str {
        match self {
            AnthologyChild::Item(item) => item.slug(),
            AnthologyChild::Shelf(shelf) => &shelf.slug,
        }
    }
}

/// A page-bearing anthology-level grouping. Serialized with a `kind` tag
/// so templates can dispatch on it.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum AnthologyItem {
    Topic {
        #[serde(flatten)]
        topic: Topic,
    },
    Book {
        #[serde(flatten)]
        book: Book,
    },
    /// Anthologies nest: an anthology can hold further anthologies.
    Anthology {
        #[serde(flatten)]
        anthology: Anthology,
    },
}

impl AnthologyItem {
    pub fn order(&self) -> u32 {
        match self {
            AnthologyItem::Topic { topic } => topic.order,
            AnthologyItem::Book { book } => book.order,
            AnthologyItem::Anthology { anthology } => anthology.order,
        }
    }

    pub fn slug(&self) -> &str {
        match self {
            AnthologyItem::Topic { topic } => &topic.slug,
            AnthologyItem::Book { book } => &book.slug,
            AnthologyItem::Anthology { anthology } => &anthology.slug,
        }
    }
}

/// An anthology's `trail.toml`. Unknown keys are load errors.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AnthologyConfig {
    title: String,
    description: String,
    #[serde(default)]
    inline_ref: Vec<String>,
}

/// A topic's, shelf's, subfolder's or chapter's *optional* `trail.toml`:
/// an explicit title, for casing the derived default can't produce
/// (acronyms and such), and — everywhere but shelves — inline_ref
/// phrase claims.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TitleConfig {
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    inline_ref: Vec<String>,
}

#[derive(Debug)]
pub struct Topic {
    pub slug: String,
    /// Site-absolute URL path; topics can sit in an anthology
    /// ("/peios/security-fundamentals/identity") or directly in a product
    /// ("/peios/impersonation"). Shelves never contribute a segment.
    pub path: String,
    pub order: u32,
    /// Derived from the slug ("the-two-gates" → "The Two Gates") unless the
    /// topic's trail.toml overrides it.
    pub title: String,
    /// Every article in this topic, added up; see `Article`.
    pub reading_minutes: u32,
    /// The most recent `updated:` date among this topic's articles.
    pub updated: Option<String>,
    /// Phrases from trail.toml (`inline_ref`) that auto-link to this
    /// topic's first article wherever prose states them.
    pub inline_refs: Vec<String>,
    /// The topic's body in `NN--` order: articles interleaved with
    /// subfolders (plain `<order>--<slug>` directories grouping articles).
    pub children: Vec<TopicChild>,
    /// `.link` entries awaiting resolution; drained into `children` as
    /// articles once the whole site is loaded.
    links: Vec<LinkStub>,
}

impl Topic {
    /// Every article in reading order, subfolder contents flattened in
    /// place — what the topic cards list and count.
    pub fn pages(&self) -> impl Iterator<Item = &Article> {
        self.children.iter().flat_map(|child| match child {
            TopicChild::Article { article } => std::slice::from_ref(article).iter(),
            TopicChild::Folder { folder } => folder.articles.iter(),
        })
    }

    /// Where topic links land: the first article in reading order.
    /// None for a topic with no articles anywhere.
    pub fn entry(&self) -> Option<&str> {
        self.pages().next().map(|article| article.path.as_str())
    }
}

// Serialized by hand so templates get the computed views alongside the
// tree: `pages` (flattened reading order, for cards) and `entry` (where
// topic links land) — without storing duplicate articles in the model.
impl Serialize for Topic {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("Topic", 9)?;
        s.serialize_field("slug", &self.slug)?;
        s.serialize_field("path", &self.path)?;
        s.serialize_field("order", &self.order)?;
        s.serialize_field("title", &self.title)?;
        s.serialize_field("reading_minutes", &self.reading_minutes)?;
        s.serialize_field("updated", &self.updated)?;
        s.serialize_field("entry", &self.entry())?;
        s.serialize_field("children", &self.children)?;
        s.serialize_field("pages", &self.pages().collect::<Vec<_>>())?;
        s.end()
    }
}

/// One entry in a topic's ordered body. Serialized with a `kind` tag so
/// templates can dispatch on it.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum TopicChild {
    Article {
        #[serde(flatten)]
        article: Article,
    },
    Folder {
        #[serde(flatten)]
        folder: TopicFolder,
    },
}

impl TopicChild {
    fn order(&self) -> u32 {
        match self {
            TopicChild::Article { article } => article.order,
            TopicChild::Folder { folder } => folder.order,
        }
    }

    fn slug(&self) -> &str {
        match self {
            TopicChild::Article { article } => &article.slug,
            TopicChild::Folder { folder } => &folder.slug,
        }
    }
}

/// A subfolder of a topic: a plain `<order>--<slug>` directory grouping a
/// run of the topic's articles under its own URL segment and sidebar
/// section. No page of its own — links open its first article. A folder
/// with no articles yet is tolerated and simply not shown.
#[derive(Debug, Clone, Serialize)]
pub struct TopicFolder {
    pub slug: String,
    /// URL path prefix of the folder's articles.
    pub path: String,
    pub order: u32,
    /// Derived from the slug unless the folder's trail.toml overrides it.
    pub title: String,
    /// The first article's path — where folder links land. None while
    /// the folder is empty.
    pub entry: Option<String>,
    /// Every article in this folder, added up; see `Article`.
    pub reading_minutes: u32,
    /// The most recent `updated:` date among this folder's articles.
    pub updated: Option<String>,
    /// Phrases from trail.toml (`inline_ref`) that auto-link to this
    /// folder's first article wherever prose states them.
    #[serde(skip)]
    pub inline_refs: Vec<String>,
    /// The folder's articles in `NN--` order.
    pub articles: Vec<Article>,
    /// `.link` entries awaiting resolution; drained into `articles` once
    /// the whole site is loaded.
    #[serde(skip)]
    links: Vec<LinkStub>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Article {
    pub slug: String,
    /// Site-absolute URL path of the article page.
    pub path: String,
    pub order: u32,
    pub title: String,
    /// The article's section number within a book ("2.1"), dotted from the
    /// authored `NN--` orders along its chapter trail — or lettered
    /// ("A", "B.2") for appendix entries. None outside books.
    pub number: Option<String>,
    /// Whether this is an appendix entry (`a<N>--` order prefix): lettered
    /// instead of numbered, sorted after every numbered sibling, and shown
    /// with a full "Appendix A" label where the entry itself is displayed.
    /// Book-root only; always false outside books.
    pub appendix: bool,
    /// The page taxonomy label from frontmatter (`type:`), e.g. "concept".
    /// None inside books, where the taxonomy is meaningless.
    pub kind: Option<String>,
    /// Optional summary, fed to search-result snippets and social
    /// previews. Book articles usually go without; `--strict` requires
    /// one on every article.
    pub description: Option<String>,
    /// Unresolved cross-reference slugs from frontmatter; resolution comes
    /// with the linking layer.
    pub related: Vec<String>,
    /// Phrases from frontmatter (`inline_ref:`) that auto-link to this
    /// article wherever prose states them.
    #[serde(skip)]
    pub inline_refs: Vec<String>,
    /// When the article last changed (`updated:`, an ISO date). Manual:
    /// file mtimes are noise, and nothing here knows about git yet.
    pub updated: Option<String>,
    /// Estimated reading time, from the body's word count unless
    /// `reading_minutes:` overrides it. Containers add theirs up, so a
    /// book card can say how long the whole thing takes.
    pub reading_minutes: u32,
    /// For a page created by a `.link` reference: the path of the
    /// canonical article whose content this page re-renders. Linked
    /// pages stay out of the search index — the original covers it.
    pub original: Option<String>,
    /// The markdown body after the frontmatter. Not exposed to templates —
    /// it is rendered separately and passed to the article page as HTML.
    #[serde(skip)]
    pub body: String,
    /// The directory the article's .md file was loaded from — what its
    /// relative image destinations resolve against. A linked page keeps
    /// its target's source directory, so the body's images keep working.
    #[serde(skip)]
    pub source_dir: PathBuf,
    /// The article's own .md source file, for "Edit this page" links.
    /// A linked page keeps its target's file — editing the alias edits
    /// the real source.
    #[serde(skip)]
    pub source_file: PathBuf,
}

/// An article's YAML frontmatter. Unknown keys are load errors.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArticleConfig {
    title: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    related: Vec<String>,
    #[serde(default)]
    inline_ref: Vec<String>,
    #[serde(default)]
    updated: Option<String>,
    #[serde(default)]
    reading_minutes: Option<u32>,
}

/// A book article's YAML frontmatter: a title, optionally a description
/// (for search snippets and social previews). The learn taxonomy
/// (`type:`) is meaningless inside a formal document, so providing it
/// is an error.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BookArticleConfig {
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    related: Vec<String>,
    #[serde(default)]
    inline_ref: Vec<String>,
    #[serde(default)]
    updated: Option<String>,
    #[serde(default)]
    reading_minutes: Option<u32>,
}

/// A book: a formal, ordered document (a specification, a TRM) with its
/// own cover page. Its body is one ordered sequence of articles and
/// chapters; chapters nest arbitrarily deep.
#[derive(Debug, Serialize)]
pub struct Book {
    pub slug: String,
    /// Site-absolute URL path of the cover page, e.g. "/demo/pgss".
    pub path: String,
    pub order: u32,
    pub title: String,
    /// Optional short name (usually an acronym, "PGSS") shown where the
    /// full title is too long: breadcrumbs and the sidebar heading.
    pub short: Option<String>,
    pub description: String,
    /// Every article in this book, added up; see `Article`.
    pub reading_minutes: u32,
    /// The most recent `updated:` date anywhere in this book.
    pub updated: Option<String>,
    /// Phrases from trail.toml (`inline_ref`) that auto-link to this
    /// book's cover — optionally reaching a section via a `§` suffix in
    /// prose ("PGSS §2.4").
    #[serde(skip)]
    pub inline_refs: Vec<String>,
    /// Direct children in `NN--` order. Serialized: templates walk the
    /// tree for the cover's contents and the article sidebar.
    pub children: Vec<BookChild>,
    /// `.link` entries awaiting resolution; drained into `children` as
    /// numbered alias articles once the whole site is loaded.
    #[serde(skip)]
    links: Vec<LinkStub>,
}

impl Book {
    /// Every article in reading order, each with its chapter trail.
    pub fn articles(&self) -> Vec<(Vec<&Chapter>, &Article)> {
        fn walk<'a>(
            children: &'a [BookChild],
            trail: &mut Vec<&'a Chapter>,
            out: &mut Vec<(Vec<&'a Chapter>, &'a Article)>,
        ) {
            for child in children {
                match child {
                    BookChild::Article { article } => out.push((trail.clone(), article)),
                    BookChild::Chapter { chapter } => {
                        trail.push(chapter);
                        walk(&chapter.children, trail, out);
                        trail.pop();
                    }
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.children, &mut Vec::new(), &mut out);
        out
    }
}

/// One entry in a book's or chapter's ordered body. Serialized with a
/// `kind` tag so templates can dispatch on it.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum BookChild {
    Article {
        #[serde(flatten)]
        article: Article,
    },
    Chapter {
        #[serde(flatten)]
        chapter: Chapter,
    },
}

impl BookChild {
    fn order(&self) -> u32 {
        match self {
            BookChild::Article { article } => article.order,
            BookChild::Chapter { chapter } => chapter.order,
        }
    }

    fn appendix(&self) -> bool {
        match self {
            BookChild::Article { article } => article.appendix,
            BookChild::Chapter { chapter } => chapter.appendix,
        }
    }

    /// The order prefix as authored ("3", "a1"), for duplicate reports.
    fn order_label(&self) -> String {
        if self.appendix() {
            format!("a{}", self.order())
        } else {
            self.order().to_string()
        }
    }

    fn slug(&self) -> &str {
        match self {
            BookChild::Article { article } => &article.slug,
            BookChild::Chapter { chapter } => &chapter.slug,
        }
    }
}

/// A chapter of a book: a plain `<order>--<slug>` directory. Chapters have
/// no page of their own — links to a chapter open its first article.
#[derive(Debug, Serialize)]
pub struct Chapter {
    pub slug: String,
    /// URL path prefix of the chapter's articles. There is no page here;
    /// templates use it to tell which branches contain the current page.
    pub path: String,
    pub order: u32,
    /// The chapter's section number ("2.2"), dotted from the authored
    /// `NN--` orders along its trail — gaps show through on purpose.
    /// Appendix chapters are lettered ("A") instead.
    pub number: String,
    /// Whether this is an appendix chapter; see `Article::appendix`.
    pub appendix: bool,
    /// Derived from the slug unless the chapter's trail.toml overrides it.
    pub title: String,
    /// URL path of the chapter's first article in reading order — where
    /// chapter links (contents, breadcrumbs, sidebar) land.
    pub entry: String,
    /// Every article in this chapter, added up; see `Article`.
    pub reading_minutes: u32,
    /// The most recent `updated:` date anywhere in this chapter.
    pub updated: Option<String>,
    /// Phrases from trail.toml (`inline_ref`) that auto-link to this
    /// chapter's first article wherever prose states them.
    #[serde(skip)]
    pub inline_refs: Vec<String>,
    pub children: Vec<BookChild>,
    /// `.link` entries awaiting resolution; see `Book::links`.
    #[serde(skip)]
    links: Vec<LinkStub>,
}

/// A book's `trail.toml`. Unknown keys are load errors.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BookConfig {
    title: String,
    short: Option<String>,
    description: String,
    #[serde(default)]
    inline_ref: Vec<String>,
}

/// A `.link` reference file: `target` is a `~` reference in the same
/// grammar articles use; `title` overrides the slug-derived default.
/// Unknown keys are load errors.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LinkConfig {
    target: String,
    title: Option<String>,
}

/// An unresolved `.link` entry, held until the whole site has loaded —
/// its target may live anywhere in the tree.
#[derive(Debug, Clone)]
pub struct LinkStub {
    slug: String,
    order: u32,
    /// Book links may use `a<N>--` orders, becoming appendix entries.
    appendix: bool,
    title: String,
    target: String,
}

/// A parsed grouping directory name: `<order>--<slug>.<kind>`.
struct GroupingName {
    order: u32,
    slug: String,
    kind: String,
}

impl Site {
    /// Load a site from `root`. `out` is the build output directory, which is
    /// tolerated inside the root without counting as content.
    ///
    /// The root may contain only `trail.toml` and `*.product` directories;
    /// anything else (dot-entries aside) is an error, so typos surface at
    /// build time instead of silently vanishing from the site.
    pub fn load(root: &Path, out: &Path) -> Result<Site> {
        let mut config: SiteConfig = read_toml(&root.join("trail.toml"))?;
        if let Some(url) = &mut config.url {
            while url.ends_with('/') {
                url.pop();
            }
        }
        if let Some(accent) = &config.accent {
            validate_color(accent).context("in the site accent")?;
        }
        if let Some(accent_dark) = &config.accent_dark {
            ensure!(
                config.accent.is_some(),
                "accent_dark without accent: set the base accent color too"
            );
            validate_color(accent_dark).context("in the site accent_dark")?;
        }
        let custom_css = match &config.custom_css {
            Some(path) => {
                let file = root.join(path);
                ensure!(
                    file.is_file(),
                    "custom_css names '{path}', which does not exist in the site root"
                );
                Some(file)
            }
            None => None,
        };
        let favicon = match &config.favicon {
            Some(path) => {
                let file = root.join(path);
                ensure!(
                    file.is_file(),
                    "favicon names '{path}', which does not exist in the site root"
                );
                Some(file)
            }
            None => None,
        };
        let head_html = match &config.head_html {
            Some(path) => {
                let file = root.join(path);
                ensure!(
                    file.is_file(),
                    "head_html names '{path}', which does not exist in the site root"
                );
                Some(fs::read_to_string(&file).with_context(|| format!("reading {path}"))?)
            }
            None => None,
        };
        if let Some(template) = &config.edit_url {
            ensure!(
                template.contains("{path}"),
                "edit_url must contain a {{path}} placeholder for the \
                 article's source path"
            );
        }
        for entry in &config.passthrough {
            validate_passthrough(entry, root, out)?;
        }
        // Configured root files (or the directories holding them) are
        // content the root scan below must tolerate.
        let keep_entries: Vec<String> = [&config.custom_css, &config.favicon, &config.head_html]
            .into_iter()
            .flatten()
            .chain(&config.passthrough)
            .filter_map(|path| Path::new(path).components().next())
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect();

        let mut products = Vec::new();
        let mut images = Vec::new();
        for entry in read_dir_sorted(root)? {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.')
                || is_same_path(&entry.path(), out)
                || keep_entries.contains(&name)
            {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                let Some(slug) = name.strip_suffix(".product") else {
                    bail!(
                        "unexpected directory '{name}' in site root: \
                         only *.product directories are allowed"
                    );
                };
                validate_slug(slug).with_context(|| format!("in directory '{name}'"))?;
                ensure!(
                    slug != "assets" && slug != "pagefind",
                    "'{slug}' is a reserved product slug (it collides with the \
                     '/{slug}' output directory)"
                );
                products.push(
                    load_product(&path, slug, &mut images)
                        .with_context(|| format!("loading product '{slug}'"))?,
                );
            } else if name != "trail.toml" {
                bail!("unexpected file '{name}' in site root: only trail.toml is allowed");
            }
        }

        for slug in &config.featured {
            ensure!(
                products.iter().any(|p| p.slug == *slug),
                "featured product '{slug}' does not exist (no {slug}.product directory)"
            );
        }

        let rank = |p: &Product| config.featured.iter().position(|s| *s == p.slug);
        products.sort_by(|a, b| match (rank(a), rank(b)) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
        });

        let mut site = Site {
            config,
            products,
            images,
            custom_css,
            favicon,
            head_html,
            root: root.to_path_buf(),
        };
        resolve_linked_articles(&mut site)?;
        // Reading times and dates roll up from the leaves, so this runs
        // after `.link` aliases have joined their containers.
        for product in &mut site.products {
            rollup_product(product);
        }

        // Header nav links: external URLs pass through; `~` references
        // resolve through the body-link grammar — strictly, at load,
        // because site chrome must never dangle.
        let resolved = {
            let index = crate::links::LinkIndex::new(&site);
            let mut resolved = Vec::with_capacity(site.config.nav.len());
            for item in &site.config.nav {
                resolved.push(match item.url.strip_prefix('~') {
                    Some(reference) => Some(index.resolve(reference).with_context(|| {
                        format!("resolving nav link '{}' ({})", item.label, item.url)
                    })?),
                    None => None,
                });
            }
            resolved
        };
        for (item, url) in site.config.nav.iter_mut().zip(resolved) {
            if let Some(url) = url {
                item.url = url;
            }
        }
        Ok(site)
    }

    /// The products shown as cards on the front page, in `featured` order.
    pub fn featured(&self) -> Vec<&Product> {
        self.config
            .featured
            .iter()
            .filter_map(|slug| self.products.iter().find(|p| p.slug == *slug))
            .collect()
    }

    /// The served favicon URL: the configured file's own name at the
    /// output root.
    pub fn favicon_href(&self) -> Option<String> {
        self.favicon.as_ref().map(|source| {
            format!(
                "/{}",
                source
                    .file_name()
                    .expect("validated as a file at load")
                    .to_string_lossy()
            )
        })
    }

    /// Every article in the site in tree order — topic articles
    /// (subfolders included), book articles, aliases and all.
    pub fn articles(&self) -> Vec<&Article> {
        fn from_anthology<'a>(anthology: &'a Anthology, out: &mut Vec<&'a Article>) {
            for item in anthology.items() {
                match item {
                    AnthologyItem::Topic { topic } => out.extend(topic.pages()),
                    AnthologyItem::Book { book } => {
                        out.extend(book.articles().into_iter().map(|(_, article)| article));
                    }
                    AnthologyItem::Anthology { anthology } => from_anthology(anthology, out),
                }
            }
        }
        let mut out = Vec::new();
        for product in &self.products {
            for item in product.items() {
                match item {
                    ProductItem::Topic { topic } => out.extend(topic.pages()),
                    ProductItem::Book { book } => {
                        out.extend(book.articles().into_iter().map(|(_, article)| article));
                    }
                    ProductItem::Anthology { anthology } => from_anthology(anthology, &mut out),
                }
            }
        }
        out
    }
}

fn load_product(dir: &Path, slug: &str, images: &mut Vec<ImageAsset>) -> Result<Product> {
    let config: ProductConfig = read_toml(&dir.join("trail.toml"))?;
    validate_color(&config.color)?;
    let path = format!("/{slug}");

    let mut children = Vec::new();
    for (child_dir, name) in grouping_dirs(dir, "a product")? {
        let grouping = parse_grouping_name(&name)?;
        let child = match grouping.kind.as_str() {
            "antho" => ProductChild::Item(ProductItem::Anthology {
                anthology: load_anthology(&child_dir, grouping, &path, images)
                    .with_context(|| format!("loading anthology in '{name}'"))?,
            }),
            "topic" => ProductChild::Item(ProductItem::Topic {
                topic: load_topic(&child_dir, grouping, &path, images)
                    .with_context(|| format!("loading topic in '{name}'"))?,
            }),
            "book" => ProductChild::Item(ProductItem::Book {
                book: load_book(&child_dir, grouping, &path, images)
                    .with_context(|| format!("loading book in '{name}'"))?,
            }),
            "shelf" => ProductChild::Shelf(
                load_product_shelf(&child_dir, grouping, &path, images)
                    .with_context(|| format!("loading shelf in '{name}'"))?,
            ),
            kind => bail!(
                "unexpected grouping type '.{kind}' on '{name}' in a product \
                 (expected .antho, .book, .topic or .shelf)"
            ),
        };
        children.push(child);
    }
    children.sort_by(|a, b| {
        a.order()
            .cmp(&b.order())
            .then_with(|| a.slug().cmp(b.slug()))
    });
    check_duplicates(
        "grouping",
        &children,
        ProductChild::order,
        ProductChild::slug,
    )?;

    let product = Product {
        slug: slug.to_string(),
        path,
        title: config.title,
        monogram: config.monogram,
        color: config.color,
        description: config.description,
        reading_minutes: 0,
        updated: None,
        inline_refs: config.inline_ref,
        children,
    };
    check_flat_slugs("grouping", product.items().map(ProductItem::slug))?;
    Ok(product)
}

fn load_product_shelf(
    dir: &Path,
    name: GroupingName,
    product_path: &str,
    images: &mut Vec<ImageAsset>,
) -> Result<Shelf<ProductItem>> {
    let config = grouping_config(dir, &name.slug)?;
    ensure!(
        config.inline_refs.is_empty(),
        "inline_ref on shelf '{}': a shelf has no page — declare it on \
         the shelf's items instead",
        name.slug
    );
    let title = config.title;

    let mut items = Vec::new();
    for (child_dir, entry_name) in grouping_dirs(dir, "a shelf")? {
        let grouping = parse_grouping_name(&entry_name)?;
        let item = match grouping.kind.as_str() {
            "antho" => ProductItem::Anthology {
                anthology: load_anthology(&child_dir, grouping, product_path, images)
                    .with_context(|| format!("loading anthology in '{entry_name}'"))?,
            },
            "topic" => ProductItem::Topic {
                topic: load_topic(&child_dir, grouping, product_path, images)
                    .with_context(|| format!("loading topic in '{entry_name}'"))?,
            },
            "book" => ProductItem::Book {
                book: load_book(&child_dir, grouping, product_path, images)
                    .with_context(|| format!("loading book in '{entry_name}'"))?,
            },
            "shelf" => bail!("a shelf cannot contain another shelf ('{entry_name}')"),
            kind => bail!(
                "unexpected grouping type '.{kind}' on '{entry_name}' in a shelf \
                 (expected .antho, .book or .topic)"
            ),
        };
        items.push(item);
    }
    items.sort_by(|a, b| {
        a.order()
            .cmp(&b.order())
            .then_with(|| a.slug().cmp(b.slug()))
    });
    check_duplicates("item", &items, ProductItem::order, ProductItem::slug)?;

    Ok(Shelf {
        slug: name.slug,
        order: name.order,
        title,
        items,
    })
}

fn load_anthology(
    dir: &Path,
    name: GroupingName,
    parent_path: &str,
    images: &mut Vec<ImageAsset>,
) -> Result<Anthology> {
    let config: AnthologyConfig = read_toml(&dir.join("trail.toml"))?;
    let path = format!("{parent_path}/{}", name.slug);

    let mut children = Vec::new();
    for (child_dir, entry_name) in grouping_dirs(dir, "an anthology")? {
        let grouping = parse_grouping_name(&entry_name)?;
        let child = match grouping.kind.as_str() {
            "antho" => AnthologyChild::Item(AnthologyItem::Anthology {
                anthology: load_anthology(&child_dir, grouping, &path, images)
                    .with_context(|| format!("loading anthology in '{entry_name}'"))?,
            }),
            "topic" => AnthologyChild::Item(AnthologyItem::Topic {
                topic: load_topic(&child_dir, grouping, &path, images)
                    .with_context(|| format!("loading topic in '{entry_name}'"))?,
            }),
            "book" => AnthologyChild::Item(AnthologyItem::Book {
                book: load_book(&child_dir, grouping, &path, images)
                    .with_context(|| format!("loading book in '{entry_name}'"))?,
            }),
            "shelf" => AnthologyChild::Shelf(
                load_anthology_shelf(&child_dir, grouping, &path, images)
                    .with_context(|| format!("loading shelf in '{entry_name}'"))?,
            ),
            kind => bail!(
                "unexpected grouping type '.{kind}' on '{entry_name}' in an anthology \
                 (expected .antho, .topic, .book or .shelf)"
            ),
        };
        children.push(child);
    }
    children.sort_by(|a, b| {
        a.order()
            .cmp(&b.order())
            .then_with(|| a.slug().cmp(b.slug()))
    });
    check_duplicates(
        "grouping",
        &children,
        AnthologyChild::order,
        AnthologyChild::slug,
    )?;

    let anthology = Anthology {
        slug: name.slug,
        path,
        order: name.order,
        title: config.title,
        description: config.description,
        reading_minutes: 0,
        updated: None,
        inline_refs: config.inline_ref,
        children,
    };
    check_flat_slugs("item", anthology.items().map(AnthologyItem::slug))?;
    Ok(anthology)
}

fn load_anthology_shelf(
    dir: &Path,
    name: GroupingName,
    anthology_path: &str,
    images: &mut Vec<ImageAsset>,
) -> Result<Shelf<AnthologyItem>> {
    let config = grouping_config(dir, &name.slug)?;
    ensure!(
        config.inline_refs.is_empty(),
        "inline_ref on shelf '{}': a shelf has no page — declare it on \
         the shelf's items instead",
        name.slug
    );
    let title = config.title;

    let mut items = Vec::new();
    for (child_dir, entry_name) in grouping_dirs(dir, "a shelf")? {
        let grouping = parse_grouping_name(&entry_name)?;
        let item = match grouping.kind.as_str() {
            "antho" => AnthologyItem::Anthology {
                anthology: load_anthology(&child_dir, grouping, anthology_path, images)
                    .with_context(|| format!("loading anthology in '{entry_name}'"))?,
            },
            "topic" => AnthologyItem::Topic {
                topic: load_topic(&child_dir, grouping, anthology_path, images)
                    .with_context(|| format!("loading topic in '{entry_name}'"))?,
            },
            "book" => AnthologyItem::Book {
                book: load_book(&child_dir, grouping, anthology_path, images)
                    .with_context(|| format!("loading book in '{entry_name}'"))?,
            },
            "shelf" => bail!("a shelf cannot contain another shelf ('{entry_name}')"),
            kind => bail!(
                "unexpected grouping type '.{kind}' on '{entry_name}' in an anthology's shelf \
                 (expected .antho, .topic or .book)"
            ),
        };
        items.push(item);
    }
    items.sort_by(|a, b| {
        a.order()
            .cmp(&b.order())
            .then_with(|| a.slug().cmp(b.slug()))
    });
    check_duplicates("item", &items, AnthologyItem::order, AnthologyItem::slug)?;

    Ok(Shelf {
        slug: name.slug,
        order: name.order,
        title,
        items,
    })
}

fn load_topic(
    dir: &Path,
    name: GroupingName,
    parent_path: &str,
    images: &mut Vec<ImageAsset>,
) -> Result<Topic> {
    let config = grouping_config(dir, &name.slug)?;
    let path = format!("{parent_path}/{}", name.slug);

    let mut children = Vec::new();
    let mut links = Vec::new();
    for entry in read_dir_sorted(dir)? {
        let entry_name = entry.file_name().to_string_lossy().into_owned();
        if entry_name.starts_with('.') || entry_name == "trail.toml" {
            continue;
        }
        if entry.path().is_dir() {
            if let Some((_, kind)) = entry_name.rsplit_once('.') {
                bail!(
                    "unexpected grouping type '.{kind}' on '{entry_name}' in a topic: \
                     subfolders are plain '<order>--<slug>' directories"
                );
            }
            let Some((order, slug)) = entry_name.split_once("--") else {
                bail!("subfolder '{entry_name}' is missing its '<order>--' prefix");
            };
            let order: u32 = order.parse().with_context(|| {
                format!("subfolder '{entry_name}' has a non-numeric order prefix")
            })?;
            validate_slug(slug).with_context(|| format!("in subfolder '{entry_name}'"))?;
            children.push(TopicChild::Folder {
                folder: load_topic_folder(&entry.path(), order, slug, &path, images)
                    .with_context(|| format!("loading subfolder in '{entry_name}'"))?,
            });
        } else if let Some(asset) = image_asset(&entry_name, entry.path(), &path)? {
            images.push(asset);
        } else if let Some(stem) = entry_name.strip_suffix(".link") {
            links.push(load_link_stub(&entry.path(), stem, false)?);
        } else {
            let Some(stem) = entry_name.strip_suffix(".md") else {
                bail!(
                    "unexpected file '{entry_name}' in a topic: \
                     articles are *.md files (or *.link references)"
                );
            };
            children.push(TopicChild::Article {
                article: load_article(&entry.path(), stem, &path, None)?,
            });
        }
    }
    children.sort_by(|a, b| {
        a.order()
            .cmp(&b.order())
            .then_with(|| a.slug().cmp(b.slug()))
    });
    // Articles and subfolders share the topic's URL space, so slugs are
    // checked across both. (Link slugs join the check once resolved.)
    check_duplicates("item", &children, TopicChild::order, TopicChild::slug)?;

    Ok(Topic {
        slug: name.slug,
        path,
        order: name.order,
        title: config.title,
        reading_minutes: 0,
        updated: None,
        inline_refs: config.inline_refs,
        children,
        links,
    })
}

fn load_topic_folder(
    dir: &Path,
    order: u32,
    slug: &str,
    parent_path: &str,
    images: &mut Vec<ImageAsset>,
) -> Result<TopicFolder> {
    let config = grouping_config(dir, slug)?;
    let path = format!("{parent_path}/{slug}");

    let mut articles = Vec::new();
    let mut links = Vec::new();
    for entry in read_dir_sorted(dir)? {
        let entry_name = entry.file_name().to_string_lossy().into_owned();
        if entry_name.starts_with('.') || entry_name == "trail.toml" {
            continue;
        }
        ensure!(
            entry.path().is_file(),
            "unexpected directory '{entry_name}' in a topic subfolder: \
             subfolders hold only articles"
        );
        if let Some(asset) = image_asset(&entry_name, entry.path(), &path)? {
            images.push(asset);
            continue;
        }
        if let Some(stem) = entry_name.strip_suffix(".link") {
            links.push(load_link_stub(&entry.path(), stem, false)?);
            continue;
        }
        let Some(stem) = entry_name.strip_suffix(".md") else {
            bail!(
                "unexpected file '{entry_name}' in a topic subfolder: \
                 articles are *.md files (or *.link references)"
            );
        };
        articles.push(load_article(&entry.path(), stem, &path, None)?);
    }
    articles.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.slug.cmp(&b.slug)));
    check_duplicates("article", &articles, |a| a.order, |a| &a.slug)?;

    Ok(TopicFolder {
        slug: slug.to_string(),
        path,
        order,
        title: config.title,
        entry: articles.first().map(|article| article.path.clone()),
        reading_minutes: 0,
        updated: None,
        inline_refs: config.inline_refs,
        articles,
        links,
    })
}

/// Parse a book child's order token: plain digits ("3--install") or an
/// appendix marker ("a2--prior-art"). Appendices sort after every
/// numbered sibling at their own level — a book's appendices close out
/// the book, a chapter's close out the chapter ("2.A") — and are
/// lettered instead of numbered. Returns (appendix, order).
fn parse_book_order(token: &str, what: &str, entry_name: &str) -> Result<(bool, u32)> {
    match token.strip_prefix('a') {
        Some(rest) => {
            let order: u32 = rest.parse().with_context(|| {
                format!("{what} '{entry_name}' has a malformed appendix order prefix")
            })?;
            ensure!(
                order >= 1,
                "{what} '{entry_name}': appendix orders start at 'a1'"
            );
            Ok((true, order))
        }
        None => {
            let order: u32 = token
                .parse()
                .with_context(|| format!("{what} '{entry_name}' has a non-numeric order prefix"))?;
            Ok((false, order))
        }
    }
}

/// Appendix section letters: a1 → "A", a26 → "Z", a27 → "AA" — gaps show
/// through, like numeric orders.
fn appendix_letters(mut n: u32) -> String {
    let mut letters = Vec::new();
    while n > 0 {
        n -= 1;
        letters.push(b'A' + (n % 26) as u8);
        n /= 26;
    }
    letters.reverse();
    String::from_utf8(letters).expect("ASCII letters")
}

/// Recognise a co-located image file: `<stem>.<image-extension>`, kept
/// under the containing directory's URL with its file name intact.
/// Returns None for anything that isn't an image, leaving the caller's
/// own "unexpected file" checks to run.
fn image_asset(entry_name: &str, source: PathBuf, parent_path: &str) -> Result<Option<ImageAsset>> {
    let Some((stem, extension)) = entry_name.rsplit_once('.') else {
        return Ok(None);
    };
    if IMAGE_EXTENSIONS.contains(&extension) {
        validate_slug(stem).with_context(|| format!("in image '{entry_name}'"))?;
        return Ok(Some(ImageAsset {
            source,
            url: format!("{parent_path}/{entry_name}"),
        }));
    }
    // Catch "Diagram.PNG" here with a pointed message rather than letting
    // it fall through to the generic unexpected-file error.
    ensure!(
        !IMAGE_EXTENSIONS
            .iter()
            .any(|known| extension.eq_ignore_ascii_case(known)),
        "image '{entry_name}' must use a lowercase extension"
    );
    Ok(None)
}

/// Parse a `<order>--<slug>.link` reference file. Inside a book the
/// order may be an `a<N>--` appendix marker, like any other book entry.
fn load_link_stub(file: &Path, stem: &str, in_book: bool) -> Result<LinkStub> {
    let Some((token, slug)) = stem.split_once("--") else {
        bail!("link '{stem}.link' is missing its '<order>--' prefix");
    };
    let entry_name = format!("{stem}.link");
    let (appendix, order) = if in_book {
        parse_book_order(token, "link", &entry_name)?
    } else {
        (
            false,
            token
                .parse()
                .with_context(|| format!("link '{entry_name}' has a non-numeric order prefix"))?,
        )
    };
    validate_slug(slug).with_context(|| format!("in link '{stem}.link'"))?;
    let config: LinkConfig = read_toml(file)?;
    ensure!(
        config.target.starts_with('~'),
        "link '{stem}.link' target '{}' is not a ~reference",
        config.target
    );
    Ok(LinkStub {
        slug: slug.to_string(),
        order,
        appendix,
        title: config.title.unwrap_or_else(|| title_from_slug(slug)),
        target: config.target,
    })
}

fn load_book(
    dir: &Path,
    name: GroupingName,
    parent_path: &str,
    images: &mut Vec<ImageAsset>,
) -> Result<Book> {
    let config: BookConfig = read_toml(&dir.join("trail.toml"))?;
    let path = format!("{parent_path}/{}", name.slug);
    let (children, links) = load_book_children(dir, &path, "", "a book", images)?;

    Ok(Book {
        slug: name.slug,
        path,
        order: name.order,
        title: config.title,
        short: config.short,
        description: config.description,
        reading_minutes: 0,
        updated: None,
        inline_refs: config.inline_ref,
        children,
        links,
    })
}

#[allow(clippy::too_many_arguments)]
fn load_chapter(
    dir: &Path,
    order: u32,
    appendix: bool,
    slug: &str,
    parent_path: &str,
    number: String,
    images: &mut Vec<ImageAsset>,
) -> Result<Chapter> {
    let config = grouping_config(dir, slug)?;
    let path = format!("{parent_path}/{slug}");
    let (children, links) =
        load_book_children(dir, &path, &format!("{number}."), "a chapter", images)?;

    // Chapter links open the chapter's first article, so a chapter with
    // nothing to open is a mistake, not an empty page. (A chapter whose
    // only entries are .link references gets its entry at resolution.)
    let entry = match children.first() {
        Some(BookChild::Article { article }) => article.path.clone(),
        Some(BookChild::Chapter { chapter }) => chapter.entry.clone(),
        None if !links.is_empty() => String::new(),
        None => bail!("chapter '{slug}' contains no articles"),
    };

    Ok(Chapter {
        slug: slug.to_string(),
        path,
        order,
        number,
        appendix,
        title: config.title,
        entry,
        reading_minutes: 0,
        updated: None,
        inline_refs: config.inline_refs,
        children,
        links,
    })
}

/// The ordered body shared by books and chapters: `<order>--<slug>.md`
/// articles interleaved with plain `<order>--<slug>` chapter directories.
/// `number_prefix` is the dotted section-number prefix this level's orders
/// append to ("" for a book's root, "2." inside its chapter 2, ...).
fn load_book_children(
    dir: &Path,
    parent_path: &str,
    number_prefix: &str,
    what: &str,
    images: &mut Vec<ImageAsset>,
) -> Result<(Vec<BookChild>, Vec<LinkStub>)> {
    let mut children = Vec::new();
    let mut links = Vec::new();
    for entry in read_dir_sorted(dir)? {
        let entry_name = entry.file_name().to_string_lossy().into_owned();
        if entry_name.starts_with('.') || entry_name == "trail.toml" {
            continue;
        }
        if entry.path().is_dir() {
            if let Some((_, kind)) = entry_name.rsplit_once('.') {
                bail!(
                    "unexpected grouping type '.{kind}' on '{entry_name}' in {what}: \
                     chapters are plain '<order>--<slug>' directories"
                );
            }
            let Some((token, slug)) = entry_name.split_once("--") else {
                bail!("chapter '{entry_name}' is missing its '<order>--' prefix");
            };
            let (appendix, order) = parse_book_order(token, "chapter", &entry_name)?;
            validate_slug(slug).with_context(|| format!("in chapter '{entry_name}'"))?;
            let segment = if appendix {
                appendix_letters(order)
            } else {
                order.to_string()
            };
            let number = format!("{number_prefix}{segment}");
            children.push(BookChild::Chapter {
                chapter: load_chapter(
                    &entry.path(),
                    order,
                    appendix,
                    slug,
                    parent_path,
                    number,
                    images,
                )
                .with_context(|| format!("loading chapter in '{entry_name}'"))?,
            });
        } else if let Some(asset) = image_asset(&entry_name, entry.path(), parent_path)? {
            images.push(asset);
        } else if let Some(stem) = entry_name.strip_suffix(".link") {
            links.push(load_link_stub(&entry.path(), stem, true)?);
        } else {
            let Some(stem) = entry_name.strip_suffix(".md") else {
                bail!(
                    "unexpected file '{entry_name}' in {what}: \
                     articles are *.md files (or *.link references)"
                );
            };
            children.push(BookChild::Article {
                article: load_article(&entry.path(), stem, parent_path, Some(number_prefix))?,
            });
        }
    }
    sort_book_children(&mut children);
    // Articles and chapters share the book's URL space, so slugs are
    // checked across both. (Link slugs join the check once resolved.)
    check_duplicates("item", &children, BookChild::order_label, BookChild::slug)?;
    Ok((children, links))
}

/// Appendices come after every numbered entry, whatever their orders.
fn sort_book_children(children: &mut [BookChild]) {
    children.sort_by(|a, b| {
        (a.appendix(), a.order())
            .cmp(&(b.appendix(), b.order()))
            .then_with(|| a.slug().cmp(b.slug()))
    });
}

/// Load a `<order>--<slug>.md` article; `stem` is the filename without
/// the `.md`. `number_prefix` is the section-number prefix inside a book
/// (see `load_book_children`) and doubles as the signal that this is a
/// book article (title-only frontmatter); topic articles pass None.
fn load_article(
    file: &Path,
    stem: &str,
    parent_path: &str,
    number_prefix: Option<&str>,
) -> Result<Article> {
    let Some((token, slug)) = stem.split_once("--") else {
        bail!("article '{stem}.md' is missing its '<order>--' prefix");
    };
    // Appendix (`a<N>--`) entries exist only inside books; a topic
    // article's order stays plain digits.
    let entry_name = format!("{stem}.md");
    let (appendix, order) = match number_prefix {
        Some(_) => parse_book_order(token, "article", &entry_name)?,
        None => (
            false,
            token.parse().with_context(|| {
                format!("article '{entry_name}' has a non-numeric order prefix")
            })?,
        ),
    };
    validate_slug(slug).with_context(|| format!("in article '{stem}.md'"))?;

    let context = || format!("loading article '{slug}'");
    let (title, kind, description, related, inline_refs, updated, minutes, body) =
        match number_prefix {
            Some(_) => {
                let (frontmatter, body): (BookArticleConfig, _) =
                    read_article(file).with_context(context)?;
                (
                    frontmatter.title,
                    None,
                    frontmatter.description,
                    frontmatter.related,
                    frontmatter.inline_ref,
                    frontmatter.updated,
                    frontmatter.reading_minutes,
                    body,
                )
            }
            None => {
                let (frontmatter, body): (ArticleConfig, _) =
                    read_article(file).with_context(context)?;
                (
                    frontmatter.title,
                    Some(frontmatter.kind),
                    frontmatter.description,
                    frontmatter.related,
                    frontmatter.inline_ref,
                    frontmatter.updated,
                    frontmatter.reading_minutes,
                    body,
                )
            }
        };
    if let Some(updated) = &updated {
        validate_date(updated).with_context(|| format!("in article '{stem}.md'"))?;
    }
    let reading_minutes = minutes.unwrap_or_else(|| reading_minutes(&body));
    let segment = if appendix {
        appendix_letters(order)
    } else {
        order.to_string()
    };
    Ok(Article {
        slug: slug.to_string(),
        path: format!("{parent_path}/{slug}"),
        order,
        number: number_prefix.map(|prefix| format!("{prefix}{segment}")),
        appendix,
        title,
        kind,
        description,
        related,
        inline_refs,
        updated,
        reading_minutes,
        original: None,
        body,
        source_dir: file.parent().map(Path::to_path_buf).unwrap_or_default(),
        source_file: file.to_path_buf(),
    })
}

/// Resolve every `.link` stub into a real article: the target's content
/// under the link's own slug, title, URL, and chrome. Runs after the
/// whole tree has loaded, since targets can live anywhere; targets must
/// be articles that exist on disk — a link cannot target another link.
fn resolve_linked_articles(site: &mut Site) -> Result<()> {
    // Stubs resolve through the folder-aware index: unlike body links,
    // a stub may alias a whole topic subfolder.
    let index = crate::links::LinkIndex::with_folders(site);
    let mut originals: HashMap<String, Article> = HashMap::new();
    for product in &site.products {
        for item in product.items() {
            collect_originals(item_articles(item), &mut originals);
        }
    }

    fn item_articles(item: &ProductItem) -> Vec<&Article> {
        match item {
            ProductItem::Topic { topic } => topic.pages().collect(),
            ProductItem::Book { book } => book.articles().into_iter().map(|(_, a)| a).collect(),
            ProductItem::Anthology { anthology } => anthology
                .items()
                .flat_map(|item| anthology_item_articles(item))
                .collect(),
        }
    }
    fn anthology_item_articles(item: &AnthologyItem) -> Vec<&Article> {
        match item {
            AnthologyItem::Topic { topic } => topic.pages().collect(),
            AnthologyItem::Book { book } => book.articles().into_iter().map(|(_, a)| a).collect(),
            AnthologyItem::Anthology { anthology } => anthology
                .items()
                .flat_map(anthology_item_articles)
                .collect(),
        }
    }
    fn collect_originals<'a>(
        articles: impl IntoIterator<Item = &'a Article>,
        originals: &mut HashMap<String, Article>,
    ) {
        for article in articles {
            originals.insert(article.path.clone(), article.clone());
        }
    }

    fn topics_mut(products: &mut [Product]) -> Vec<&mut Topic> {
        fn from_anthology<'a>(anthology: &'a mut Anthology, out: &mut Vec<&'a mut Topic>) {
            for child in &mut anthology.children {
                let items: &mut dyn Iterator<Item = &mut AnthologyItem> = match child {
                    AnthologyChild::Item(item) => &mut std::iter::once(item),
                    AnthologyChild::Shelf(shelf) => &mut shelf.items.iter_mut(),
                };
                for item in items {
                    match item {
                        AnthologyItem::Topic { topic } => out.push(topic),
                        AnthologyItem::Anthology { anthology } => from_anthology(anthology, out),
                        AnthologyItem::Book { .. } => {}
                    }
                }
            }
        }
        let mut out = Vec::new();
        for product in products {
            for child in &mut product.children {
                let items: &mut dyn Iterator<Item = &mut ProductItem> = match child {
                    ProductChild::Item(item) => &mut std::iter::once(item),
                    ProductChild::Shelf(shelf) => &mut shelf.items.iter_mut(),
                };
                for item in items {
                    match item {
                        ProductItem::Topic { topic } => out.push(topic),
                        ProductItem::Anthology { anthology } => from_anthology(anthology, &mut out),
                        ProductItem::Book { .. } => {}
                    }
                }
            }
        }
        out
    }

    // Pass 1: stubs inside subfolders (always article aliases), so the
    // folder snapshots taken next are complete.
    for topic in topics_mut(&mut site.products) {
        for child in &mut topic.children {
            let TopicChild::Folder { folder } = child else {
                continue;
            };
            for stub in std::mem::take(&mut folder.links) {
                let resolved = resolve_stub(&index, &stub, &folder.path)?;
                let Some(original) = originals.get(&resolved) else {
                    bail!(
                        "link '{}' in '{}' targets '{}', which is not an article \
                         (a subfolder can only link articles, and a link cannot \
                         target another link)",
                        stub.slug,
                        folder.path,
                        stub.target
                    );
                };
                folder
                    .articles
                    .push(alias_article(original, &stub, &folder.path, resolved));
            }
            folder
                .articles
                .sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.slug.cmp(&b.slug)));
            check_duplicates("article", &folder.articles, |a| a.order, |a| &a.slug)
                .with_context(|| format!("in subfolder '{}'", folder.path))?;
            folder.entry = folder.articles.first().map(|article| article.path.clone());
        }
    }

    // Folder snapshots, for topic-level stubs that alias a whole folder.
    let mut folders: HashMap<String, TopicFolder> = HashMap::new();
    for product in &site.products {
        for item in product.items() {
            collect_folders(item, &mut folders);
        }
    }
    fn collect_folders(item: &ProductItem, folders: &mut HashMap<String, TopicFolder>) {
        fn from_topic(topic: &Topic, folders: &mut HashMap<String, TopicFolder>) {
            for child in &topic.children {
                if let TopicChild::Folder { folder } = child {
                    folders.insert(folder.path.clone(), folder.clone());
                }
            }
        }
        fn from_anthology(anthology: &Anthology, folders: &mut HashMap<String, TopicFolder>) {
            for item in anthology.items() {
                match item {
                    AnthologyItem::Topic { topic } => from_topic(topic, folders),
                    AnthologyItem::Anthology { anthology } => from_anthology(anthology, folders),
                    AnthologyItem::Book { .. } => {}
                }
            }
        }
        match item {
            ProductItem::Topic { topic } => from_topic(topic, folders),
            ProductItem::Anthology { anthology } => from_anthology(anthology, folders),
            ProductItem::Book { .. } => {}
        }
    }

    // Pass 2: topic-level stubs — an article alias, or a whole folder
    // cloned under the topic's own URL space.
    for topic in topics_mut(&mut site.products) {
        for stub in std::mem::take(&mut topic.links) {
            let resolved = resolve_stub(&index, &stub, &topic.path)?;
            if let Some(original) = originals.get(&resolved) {
                topic.children.push(TopicChild::Article {
                    article: alias_article(original, &stub, &topic.path, resolved),
                });
            } else if let Some(source) = folders.get(&resolved) {
                topic.children.push(TopicChild::Folder {
                    folder: alias_folder(source, &stub, &topic.path),
                });
            } else {
                bail!(
                    "link '{}' in '{}' targets '{}', which is not an article or a \
                     topic subfolder (a link cannot target another link)",
                    stub.slug,
                    topic.path,
                    stub.target
                );
            }
        }
        topic.children.sort_by(|a, b| {
            a.order()
                .cmp(&b.order())
                .then_with(|| a.slug().cmp(b.slug()))
        });
        check_duplicates("item", &topic.children, TopicChild::order, TopicChild::slug)
            .with_context(|| format!("in topic '{}'", topic.path))?;
    }

    // Pass 3: stubs in books and chapters become numbered alias articles.
    for book in books_mut(&mut site.products) {
        let links = std::mem::take(&mut book.links);
        resolve_book_links(
            &index,
            &originals,
            &mut book.children,
            links,
            &book.path,
            "",
        )?;
    }

    fn books_mut(products: &mut [Product]) -> Vec<&mut Book> {
        fn from_anthology<'a>(anthology: &'a mut Anthology, out: &mut Vec<&'a mut Book>) {
            for child in &mut anthology.children {
                let items: &mut dyn Iterator<Item = &mut AnthologyItem> = match child {
                    AnthologyChild::Item(item) => &mut std::iter::once(item),
                    AnthologyChild::Shelf(shelf) => &mut shelf.items.iter_mut(),
                };
                for item in items {
                    match item {
                        AnthologyItem::Book { book } => out.push(book),
                        AnthologyItem::Anthology { anthology } => from_anthology(anthology, out),
                        AnthologyItem::Topic { .. } => {}
                    }
                }
            }
        }
        let mut out = Vec::new();
        for product in products {
            for child in &mut product.children {
                let items: &mut dyn Iterator<Item = &mut ProductItem> = match child {
                    ProductChild::Item(item) => &mut std::iter::once(item),
                    ProductChild::Shelf(shelf) => &mut shelf.items.iter_mut(),
                };
                for item in items {
                    match item {
                        ProductItem::Book { book } => out.push(book),
                        ProductItem::Anthology { anthology } => from_anthology(anthology, &mut out),
                        ProductItem::Topic { .. } => {}
                    }
                }
            }
        }
        out
    }

    Ok(())
}

/// Resolve a stub's `~` target to a page (or, for topic-level stubs, a
/// folder) path, through the folder-aware index.
fn resolve_stub(
    index: &crate::links::LinkIndex,
    stub: &LinkStub,
    parent_path: &str,
) -> Result<String> {
    let reference = stub.target.trim_start_matches('~');
    index
        .resolve(reference)
        .with_context(|| format!("resolving link '{}' in '{parent_path}'", stub.slug))
}

/// The target article cloned under the link's own slug, URL, and title.
fn alias_article(
    original: &Article,
    stub: &LinkStub,
    parent_path: &str,
    resolved: String,
) -> Article {
    let mut article = original.clone();
    article.slug = stub.slug.clone();
    article.path = format!("{parent_path}/{}", stub.slug);
    article.order = stub.order;
    article.title = stub.title.clone();
    article.number = None;
    article.appendix = false;
    // Aliases never claim inline_ref phrases; the original keeps them.
    article.inline_refs = Vec::new();
    article.original = Some(resolved);
    article
}

/// The target folder cloned under the link's own slug and URL: every
/// article inside becomes an alias of its original (an article that was
/// already an alias keeps pointing at its true original).
fn alias_folder(source: &TopicFolder, stub: &LinkStub, parent_path: &str) -> TopicFolder {
    let mut folder = source.clone();
    folder.slug = stub.slug.clone();
    folder.order = stub.order;
    folder.title = stub.title.clone();
    folder.path = format!("{parent_path}/{}", stub.slug);
    folder.inline_refs = Vec::new();
    for article in &mut folder.articles {
        let canonical = article
            .original
            .take()
            .unwrap_or_else(|| article.path.clone());
        article.original = Some(canonical);
        article.inline_refs = Vec::new();
        article.path = format!("{}/{}", folder.path, article.slug);
    }
    folder.entry = folder.articles.first().map(|article| article.path.clone());
    folder
}

/// Resolve one book level's stubs into numbered alias articles, then
/// recurse into its chapters, re-sorting and re-checking each level and
/// recomputing chapter entries (a link can become the first entry).
fn resolve_book_links(
    index: &crate::links::LinkIndex,
    originals: &HashMap<String, Article>,
    children: &mut Vec<BookChild>,
    links: Vec<LinkStub>,
    level_path: &str,
    number_prefix: &str,
) -> Result<()> {
    for stub in links {
        let resolved = resolve_stub(index, &stub, level_path)?;
        let Some(original) = originals.get(&resolved) else {
            bail!(
                "link '{}' in '{level_path}' targets '{}', which is not an article \
                 (only articles can be linked into a book, and a link cannot \
                 target another link)",
                stub.slug,
                stub.target
            );
        };
        let mut article = alias_article(original, &stub, level_path, resolved);
        let segment = if stub.appendix {
            appendix_letters(stub.order)
        } else {
            stub.order.to_string()
        };
        article.number = Some(format!("{number_prefix}{segment}"));
        article.appendix = stub.appendix;
        // Book entries carry no learn taxonomy, aliases included.
        article.kind = None;
        article.description = None;
        children.push(BookChild::Article { article });
    }
    for child in children.iter_mut() {
        if let BookChild::Chapter { chapter } = child {
            let links = std::mem::take(&mut chapter.links);
            resolve_book_links(
                index,
                originals,
                &mut chapter.children,
                links,
                &chapter.path,
                &format!("{}.", chapter.number),
            )?;
            chapter.entry = match chapter.children.first() {
                Some(BookChild::Article { article }) => article.path.clone(),
                Some(BookChild::Chapter { chapter }) => chapter.entry.clone(),
                None => bail!("chapter '{}' contains no articles", chapter.slug),
            };
        }
    }
    sort_book_children(children);
    check_duplicates("item", children, BookChild::order_label, BookChild::slug)
        .with_context(|| format!("in '{level_path}'"))?;
    Ok(())
}

/// Estimated reading time for a body, at a middling 200 words a minute.
/// Code blocks count as prose: they are read, just differently.
fn reading_minutes(body: &str) -> u32 {
    (body.split_whitespace().count() as u32)
        .div_ceil(200)
        .max(1)
}

/// The later of two ISO dates, either of which may be absent.
fn later(a: Option<String>, b: Option<String>) -> Option<String> {
    match (a, b) {
        (Some(a), Some(b)) => Some(if a >= b { a } else { b }),
        (Some(only), None) | (None, Some(only)) => Some(only),
        (None, None) => None,
    }
}

/// Sum reading times and take the latest `updated:` up the tree, so a
/// book card can say how long the whole book takes and a topic can show
/// when anything in it last changed. Shelves are presentation only and
/// hold no totals of their own.
fn rollup_product(product: &mut Product) {
    let (mut minutes, mut updated) = (0, None);
    for child in &mut product.children {
        let items: &mut dyn Iterator<Item = &mut ProductItem> = match child {
            ProductChild::Item(item) => &mut std::iter::once(item),
            ProductChild::Shelf(shelf) => &mut shelf.items.iter_mut(),
        };
        for item in items {
            let totals = match item {
                ProductItem::Topic { topic } => rollup_topic(topic),
                ProductItem::Book { book } => rollup_book(book),
                ProductItem::Anthology { anthology } => rollup_anthology(anthology),
            };
            minutes += totals.0;
            updated = later(updated, totals.1);
        }
    }
    product.reading_minutes = minutes;
    product.updated = updated;
}

fn rollup_anthology(anthology: &mut Anthology) -> (u32, Option<String>) {
    let (mut minutes, mut updated) = (0, None);
    for child in &mut anthology.children {
        let items: &mut dyn Iterator<Item = &mut AnthologyItem> = match child {
            AnthologyChild::Item(item) => &mut std::iter::once(item),
            AnthologyChild::Shelf(shelf) => &mut shelf.items.iter_mut(),
        };
        for item in items {
            let totals = match item {
                AnthologyItem::Topic { topic } => rollup_topic(topic),
                AnthologyItem::Book { book } => rollup_book(book),
                AnthologyItem::Anthology { anthology } => rollup_anthology(anthology),
            };
            minutes += totals.0;
            updated = later(updated, totals.1);
        }
    }
    anthology.reading_minutes = minutes;
    anthology.updated = updated;
    (minutes, anthology.updated.clone())
}

fn rollup_topic(topic: &mut Topic) -> (u32, Option<String>) {
    let (mut minutes, mut updated) = (0, None);
    for child in &mut topic.children {
        match child {
            TopicChild::Article { article } => {
                minutes += article.reading_minutes;
                updated = later(updated, article.updated.clone());
            }
            TopicChild::Folder { folder } => {
                let (folder_minutes, folder_updated) = (
                    folder.articles.iter().map(|a| a.reading_minutes).sum(),
                    folder
                        .articles
                        .iter()
                        .fold(None, |so_far, a| later(so_far, a.updated.clone())),
                );
                folder.reading_minutes = folder_minutes;
                folder.updated = folder_updated.clone();
                minutes += folder_minutes;
                updated = later(updated, folder_updated);
            }
        }
    }
    topic.reading_minutes = minutes;
    topic.updated = updated;
    (minutes, topic.updated.clone())
}

fn rollup_book(book: &mut Book) -> (u32, Option<String>) {
    let (minutes, updated) = rollup_book_children(&mut book.children);
    book.reading_minutes = minutes;
    book.updated = updated;
    (minutes, book.updated.clone())
}

fn rollup_book_children(children: &mut [BookChild]) -> (u32, Option<String>) {
    let (mut minutes, mut updated) = (0, None);
    for child in children {
        match child {
            BookChild::Article { article } => {
                minutes += article.reading_minutes;
                updated = later(updated, article.updated.clone());
            }
            BookChild::Chapter { chapter } => {
                let (chapter_minutes, chapter_updated) =
                    rollup_book_children(&mut chapter.children);
                chapter.reading_minutes = chapter_minutes;
                chapter.updated = chapter_updated.clone();
                minutes += chapter_minutes;
                updated = later(updated, chapter_updated);
            }
        }
    }
    (minutes, updated)
}

/// What a topic's, shelf's, subfolder's or chapter's optional trail.toml
/// provides: a title (derived from the slug unless overridden) and any
/// inline_ref phrase claims.
struct GroupingConfig {
    title: String,
    inline_refs: Vec<String>,
}

fn grouping_config(dir: &Path, slug: &str) -> Result<GroupingConfig> {
    let config_path = dir.join("trail.toml");
    if !config_path.exists() {
        return Ok(GroupingConfig {
            title: title_from_slug(slug),
            inline_refs: Vec::new(),
        });
    }
    let config: TitleConfig = read_toml(&config_path)?;
    Ok(GroupingConfig {
        title: config.title.unwrap_or_else(|| title_from_slug(slug)),
        inline_refs: config.inline_ref,
    })
}

/// "the-two-gates" → "The Two Gates".
fn title_from_slug(slug: &str) -> String {
    slug.split('-')
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The subdirectories of a grouping, with files other than trail.toml rejected.
fn grouping_dirs(dir: &Path, what: &str) -> Result<Vec<(std::path::PathBuf, String)>> {
    let mut dirs = Vec::new();
    for entry in read_dir_sorted(dir)? {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if entry.path().is_dir() {
            dirs.push((entry.path(), name));
        } else if name != "trail.toml" {
            bail!("unexpected file '{name}': only trail.toml is allowed in {what}");
        }
    }
    Ok(dirs)
}

/// Parse a grouping directory name: `<order>--<slug>.<kind>`.
fn parse_grouping_name(name: &str) -> Result<GroupingName> {
    let Some((stem, kind)) = name.rsplit_once('.') else {
        bail!("grouping directory '{name}' has no type suffix (expected e.g. '.antho')");
    };
    let Some((order, slug)) = stem.split_once("--") else {
        bail!("grouping directory '{name}' is missing its '<order>--' prefix");
    };
    let order: u32 = order
        .parse()
        .with_context(|| format!("'{name}' has a non-numeric order prefix '{order}'"))?;
    validate_slug(slug).with_context(|| format!("in directory '{name}'"))?;
    Ok(GroupingName {
        order,
        slug: slug.to_string(),
        kind: kind.to_string(),
    })
}

/// Items must already be sorted by order.
fn check_duplicates<T, K: PartialEq + std::fmt::Display>(
    what: &str,
    items: &[T],
    order: impl Fn(&T) -> K,
    slug: impl Fn(&T) -> &str,
) -> Result<()> {
    for pair in items.windows(2) {
        ensure!(
            order(&pair[0]) != order(&pair[1]),
            "{what}s '{}' and '{}' share order {}",
            slug(&pair[0]),
            slug(&pair[1]),
            order(&pair[0])
        );
    }
    let mut seen = HashSet::new();
    for item in items {
        ensure!(
            seen.insert(slug(item).to_string()),
            "duplicate {what} slug '{}'",
            slug(item)
        );
    }
    Ok(())
}

/// Shelves are invisible in URLs, so their contents share the parent's
/// URL space — the filesystem can no longer guarantee uniqueness there,
/// and trail takes over that job.
fn check_flat_slugs<'a>(what: &str, slugs: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut seen = HashSet::new();
    for slug in slugs {
        ensure!(
            seen.insert(slug),
            "duplicate {what} slug '{slug}' — shelf contents share their parent's URL space"
        );
    }
    Ok(())
}

/// Split an article into its YAML frontmatter (delimited by `---` lines)
/// and its markdown body.
fn read_article<T: serde::de::DeserializeOwned>(path: &Path) -> Result<(T, String)> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let Some(rest) = text.strip_prefix("---\n") else {
        bail!(
            "{} has no frontmatter (must start with '---')",
            path.display()
        );
    };
    let Some((frontmatter, body)) = rest.split_once("\n---\n") else {
        bail!("{} has unterminated frontmatter", path.display());
    };
    let config = serde_yaml_ng::from_str(frontmatter)
        .with_context(|| format!("parsing frontmatter of {}", path.display()))?;
    Ok((config, body.trim_start().to_string()))
}

fn read_dir_sorted(dir: &Path) -> Result<Vec<fs::DirEntry>> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .and_then(Iterator::collect)
        .with_context(|| format!("reading {}", dir.display()))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    Ok(entries)
}

fn read_toml<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// `updated:` dates are ISO (YYYY-MM-DD) so they sort, roll up, and
/// feed sitemap lastmod — unlike trail 1's free-form string.
fn validate_date(date: &str) -> Result<()> {
    let parts: Vec<&str> = date.split('-').collect();
    let valid = parts.len() == 3
        && parts[0].len() == 4
        && parts[1].len() == 2
        && parts[2].len() == 2
        && parts
            .iter()
            .all(|part| part.bytes().all(|b| b.is_ascii_digit()))
        && (1..=12).contains(&parts[1].parse::<u32>().unwrap_or(0))
        && (1..=31).contains(&parts[2].parse::<u32>().unwrap_or(0));
    ensure!(
        valid,
        "updated: '{date}' is not an ISO date (expected YYYY-MM-DD)"
    );
    Ok(())
}

fn validate_slug(slug: &str) -> Result<()> {
    ensure!(
        !slug.is_empty()
            && !slug.starts_with('-')
            && !slug.ends_with('-')
            && slug
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
        "'{slug}' is not a valid slug (lowercase kebab-case)"
    );
    // Every container owns a /print single-page view, so nothing else may
    // claim that URL.
    ensure!(
        slug != "print",
        "'print' is a reserved slug (every section has a /print page)"
    );
    Ok(())
}

fn validate_color(color: &str) -> Result<()> {
    ensure!(
        color.len() == 7
            && color.starts_with('#')
            && color[1..].chars().all(|c| c.is_ascii_hexdigit()),
        "color '{color}' is not a #rrggbb hex color"
    );
    Ok(())
}

/// A `passthrough` entry: a plain relative path naming something that
/// exists in the site root. Absolute paths and `..` are rejected — a
/// passthrough copies what it names into the output, so it must not be
/// able to reach outside the site to do it.
fn validate_passthrough(entry: &str, root: &Path, out: &Path) -> Result<()> {
    ensure!(!entry.is_empty(), "passthrough entries cannot be empty");
    let path = Path::new(entry);
    ensure!(
        path.components().all(|c| matches!(c, Component::Normal(_))),
        "passthrough '{entry}' must be a plain relative path inside the site root"
    );
    let source = root.join(path);
    ensure!(
        source.exists(),
        "passthrough names '{entry}', which does not exist in the site root"
    );
    ensure!(
        !is_same_path(&source, out),
        "passthrough '{entry}' is the build output directory"
    );
    Ok(())
}

fn is_same_path(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// A valid 2×1 red PNG, so image tests exercise real header parsing
/// (and width ≠ height catches swapped dimensions).
#[cfg(test)]
pub(crate) const TEST_PNG: &[u8] = &[
    137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 2, 0, 0, 0, 1, 8, 2, 0,
    0, 0, 123, 64, 232, 221, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 0, 68, 0, 8,
    254, 1, 255, 198, 158, 121, 247, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
];

#[cfg(test)]
pub(crate) fn write_fixture(root: &Path) {
    fs::create_dir_all(root).unwrap();
    fs::write(
        root.join("trail.toml"),
        r#"
sitename = "Test Learn"
title = "Docs for testing"
description = "A fixture site."
featured = ["beta"]
footer = "footer text"
"#,
    )
    .unwrap();
    for (slug, title) in [("alpha", "Alpha"), ("beta", "Beta")] {
        let dir = root.join(format!("{slug}.product"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("trail.toml"),
            format!(
                r##"
title = "{title}"
monogram = "Xx"
color = "#3b82f6"
description = "a description"
"##
            ),
        )
        .unwrap();
    }
    // Orders 2 and 10 so a lexical sort would get them backwards. The kit
    // anthology lives inside a product-level shelf.
    for (parent, dirname, title) in [
        ("alpha.product", "10--zulu.antho", "Zulu Docs"),
        ("alpha.product", "2--acorn.antho", "Acorn Docs"),
        ("alpha.product/6--tools.shelf", "1--kit.antho", "Kit Docs"),
    ] {
        let dir = root.join(parent).join(dirname);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("trail.toml"),
            format!(
                r#"
title = "{title}"
description = "anthology description"
"#
            ),
        )
        .unwrap();
    }
    // Topics: a five-article one and a two-article one inside the acorn
    // anthology (both sides of the see-all threshold; order 12 catches lexical
    // article sorting), a bare topic directly in the product, one inside the
    // product-level shelf, and one inside an anthology-level shelf. Only the
    // wide topic has a trail.toml (title override); the others derive their
    // titles from their slugs.
    for (parent, dirname, title, articles) in [
        (
            "alpha.product/2--acorn.antho",
            "100--wide.topic",
            Some("Wide Topic"),
            &["1--a1", "2--a2", "3--a3", "4--a4", "12--a5"][..],
        ),
        (
            "alpha.product/2--acorn.antho",
            "200--narrow.topic",
            None,
            &["1--b1", "2--b2"][..],
        ),
        (
            "alpha.product",
            "5--loose.topic",
            None,
            // "a1" deliberately collides with the wide topic's a1 so link
            // tests can exercise suffix ambiguity.
            &["1--x1", "2--x2", "3--a1"][..],
        ),
        (
            "alpha.product/6--tools.shelf",
            "2--spare.topic",
            None,
            &["1--z1"][..],
        ),
        (
            "alpha.product/2--acorn.antho/300--extras.shelf",
            "10--bonus.topic",
            None,
            &["1--y1"][..],
        ),
    ] {
        let dir = root.join(parent).join(dirname);
        fs::create_dir_all(&dir).unwrap();
        if let Some(title) = title {
            fs::write(dir.join("trail.toml"), format!("title = \"{title}\"\n")).unwrap();
        }
        for article in articles {
            let slug = article.split_once("--").unwrap().1;
            fs::write(
                dir.join(format!("{article}.md")),
                format!(
                    "---\ntitle: Article {slug}\ntype: concept\ndescription: about {slug}\nrelated:\n  - alpha/b1\n  - alpha/manual\n---\n\nBody of {slug}.\n\n## First section of {slug}\n\nWords.\n"
                ),
            )
            .unwrap();
        }
    }
    // A book directly in the product: a direct article interleaved with a
    // derived-title chapter (holding a nested chapter) and a title-override
    // chapter. A second, short-less book lives inside the product shelf.
    // Book articles carry title-only frontmatter.
    let article = |slug: &str| {
        format!(
            "---\ntitle: Article {slug}\n---\n\nBody of {slug}.\n\n## First section of {slug}\n\nWords.\n\n### Detail of {slug}\n\nMore words.\n"
        )
    };
    let topic_article = |slug: &str| {
        format!(
            "---\ntitle: Article {slug}\ntype: concept\ndescription: about {slug}\n---\n\nBody of {slug}.\n\n## First section of {slug}\n\nWords.\n\n### Detail of {slug}\n\nMore words.\n"
        )
    };
    let manual = root.join("alpha.product/7--manual.book");
    fs::create_dir_all(manual.join("2--setup/2--advanced")).unwrap();
    fs::create_dir_all(manual.join("3--appendix")).unwrap();
    fs::write(
        manual.join("trail.toml"),
        "title = \"Alpha Manual\"\nshort = \"AM\"\ndescription = \"book description\"\n\
         inline_ref = [\"Alpha Manual\"]\n",
    )
    .unwrap();
    fs::write(
        manual.join("1--intro.md"),
        article("intro").replace(
            "---\n\nBody",
            "reading_minutes: 30\nupdated: 2026-05-01\n---\n\nBody",
        ) + "\n![Layout](layout.png)\n\n\
             See [install](~alpha/install#first-section-of-install) and \
             [b1](~alpha/b1).\n",
    )
    .unwrap();
    fs::write(
        manual.join("2--setup/1--install.md"),
        article("install")
            + "\n![Layout again](../layout.png)\n\nSee §2.2.1 and §A for more. \
               External citations like §9.9 stay plain.\n\n\
               The installer preserves ownership. [*install.preserves-ownership] \
               And *emphasis* in the same block must not swallow it, nor may \
               a literal `[*not.an.anchor]` in code.\n",
    )
    .unwrap();
    fs::write(
        manual.join("2--setup/2--advanced/1--tuning.md"),
        article("tuning"),
    )
    .unwrap();
    fs::write(
        manual.join("3--appendix/trail.toml"),
        "title = \"Appendix A\"\n",
    )
    .unwrap();
    fs::write(manual.join("3--appendix/1--tables.md"), article("tables")).unwrap();
    // Real appendices: an `a<N>--` article and chapter, lettered and
    // sorted after every numbered entry despite their low orders. They
    // nest too — a chapter's own appendix closes out the chapter ("2.A").
    fs::write(manual.join("a1--glossary.md"), article("glossary")).unwrap();
    fs::create_dir_all(manual.join("a2--history")).unwrap();
    fs::write(manual.join("a2--history/1--old.md"), article("old")).unwrap();
    fs::write(
        manual.join("2--setup/a1--sidenotes.md"),
        article("sidenotes"),
    )
    .unwrap();

    let guide = root.join("alpha.product/6--tools.shelf/3--field-guide.book");
    fs::create_dir_all(&guide).unwrap();
    fs::write(
        guide.join("trail.toml"),
        "title = \"Field Guide\"\ndescription = \"book description\"\n",
    )
    .unwrap();
    fs::write(guide.join("1--notes.md"), article("notes")).unwrap();

    // Anthologies hold books and further anthologies too: acorn gets a
    // book (400) and a nested anthology (500) with a topic of its own.
    let spec = root.join("alpha.product/2--acorn.antho/400--spec.book");
    fs::create_dir_all(&spec).unwrap();
    fs::write(
        spec.join("trail.toml"),
        "title = \"Acorn Spec\"\ndescription = \"spec description\"\n",
    )
    .unwrap();
    fs::write(spec.join("1--rules.md"), article("rules")).unwrap();
    let inner = root.join("alpha.product/2--acorn.antho/500--inner.antho");
    fs::create_dir_all(inner.join("1--deep.topic")).unwrap();
    fs::write(
        inner.join("trail.toml"),
        "title = \"Inner Docs\"\ndescription = \"inner description\"\n",
    )
    .unwrap();
    fs::write(inner.join("1--deep.topic/1--d1.md"), topic_article("d1")).unwrap();

    // Images live alongside the articles that use them. The loose topic
    // gets a captioned raster image and an inline SVG; the manual book
    // gets a root-level image referenced from its own level and from a
    // chapter via "../".
    fs::write(
        root.join("alpha.product/5--loose.topic/wiring.png"),
        TEST_PNG,
    )
    .unwrap();
    fs::write(
        root.join("alpha.product/5--loose.topic/glyph.svg"),
        "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>",
    )
    .unwrap();
    fs::write(
        root.join("alpha.product/5--loose.topic/20--pic.md"),
        "---\ntitle: Article pic\ntype: concept\ndescription: about pic\n---\n\n\
         ![Wiring overview](wiring.png \"The wiring\")\n\n\
         Inline ![glyph](glyph.svg) here.\n",
    )
    .unwrap();
    fs::write(manual.join("layout.png"), TEST_PNG).unwrap();

    // Topic subfolders: a populated one in the narrow topic (derived
    // title, own URL segment) and an empty one in the loose topic, which
    // is tolerated and invisible until it gains articles.
    let extra = root.join("alpha.product/2--acorn.antho/200--narrow.topic/5--extra");
    fs::create_dir_all(&extra).unwrap();
    fs::write(extra.join("1--c1.md"), topic_article("c1")).unwrap();
    fs::create_dir_all(root.join("alpha.product/5--loose.topic/9--hollow")).unwrap();

    // Links: the loose topic aliases the wide topic's a2 (derived title),
    // and the narrow topic's subfolder aliases x1 with a title override.
    fs::write(
        root.join("alpha.product/5--loose.topic/4--alias.link"),
        "target = \"~alpha/a2\"\n",
    )
    .unwrap();
    fs::write(
        extra.join("2--linked.link"),
        "target = \"~alpha/x1\"\ntitle = \"Linked X1\"\n",
    )
    .unwrap();

    // Inline references: the manual claims "Alpha Manual" (its trail.toml
    // above), x2 claims "X-Two" in frontmatter. x1's prose exercises the
    // phrase and phrase-§ forms; x2's exercises self-suppression, RFC
    // keywords, and code immunity; install (in the manual) uses bare §.
    fs::write(
        root.join("alpha.product/5--loose.topic/1--x1.md"),
        "---\ntitle: Article x1\ntype: concept\ndescription: about x1\nrelated:\n  - alpha/b1\n  - alpha/manual\n---\n\n\
         Body of x1.\n\nX-Two expands on this, and the Alpha Manual §2.2.1 tunes it.\n\n\
         ## First section of x1\n\nWords.\n",
    )
    .unwrap();
    fs::write(
        root.join("alpha.product/5--loose.topic/2--x2.md"),
        "---\ntitle: Article x2\ntype: concept\ndescription: about x2\nupdated: 2026-03-15\nrelated:\n  - alpha/b1\n  - alpha/manual\ninline_ref:\n  - X-Two\n---\n\n\
         Body of x2.\n\nX-Two stays plain here. You MUST NOT skip the Alpha Manual,\nthough you MAY skim `Alpha Manual` in code.\n\n\
         ## First section of x2\n\nWords.\n",
    )
    .unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write_fixture(dir.path());
        dir
    }

    fn load(root: &Path) -> Result<Site> {
        Site::load(root, &root.join("dist"))
    }

    fn alpha(site: &Site) -> &Product {
        site.products.iter().find(|p| p.slug == "alpha").unwrap()
    }

    fn acorn(site: &Site) -> &Anthology {
        alpha(site)
            .anthologies()
            .find(|d| d.slug == "acorn")
            .unwrap()
    }

    #[test]
    fn loads_and_orders_products_featured_first_then_by_title() {
        let dir = fixture();
        let site = load(dir.path()).unwrap();

        let slugs: Vec<_> = site.products.iter().map(|p| p.slug.as_str()).collect();
        assert_eq!(slugs, ["beta", "alpha"]);
        let featured: Vec<_> = site.featured().iter().map(|p| p.slug.as_str()).collect();
        assert_eq!(featured, ["beta"]);
    }

    #[test]
    fn flattens_shelf_items_into_display_order() {
        let dir = fixture();
        let site = load(dir.path()).unwrap();

        let items: Vec<_> = alpha(&site).items().map(ProductItem::slug).collect();
        assert_eq!(
            items,
            [
                "acorn",
                "loose",
                "kit",
                "spare",
                "field-guide",
                "manual",
                "zulu"
            ]
        );
    }

    #[test]
    fn sections_coalesce_loose_runs_and_title_shelves() {
        let dir = fixture();
        let site = load(dir.path()).unwrap();

        let alpha = alpha(&site);
        let sections = alpha.sections();
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].title, None);
        assert_eq!(
            sections[0]
                .items
                .iter()
                .map(|i| i.slug())
                .collect::<Vec<_>>(),
            ["acorn", "loose"]
        );
        assert_eq!(sections[1].title, Some("Tools"));
        assert_eq!(
            sections[1]
                .items
                .iter()
                .map(|i| i.slug())
                .collect::<Vec<_>>(),
            ["kit", "spare", "field-guide"]
        );
        assert_eq!(sections[2].title, None);
        assert_eq!(
            sections[2]
                .items
                .iter()
                .map(|i| i.slug())
                .collect::<Vec<_>>(),
            ["manual", "zulu"]
        );

        let acorn = acorn(&site);
        let sections = acorn.sections();
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[1].title, Some("Extras"));
        assert_eq!(
            sections[2]
                .items
                .iter()
                .map(|i| i.slug())
                .collect::<Vec<_>>(),
            ["spec", "inner"]
        );
    }

    #[test]
    fn shelves_are_invisible_in_urls() {
        let dir = fixture();
        let site = load(dir.path()).unwrap();

        let alpha = alpha(&site);
        assert_eq!(alpha.path, "/alpha");
        let kit = alpha.anthologies().find(|d| d.slug == "kit").unwrap();
        assert_eq!(kit.path, "/alpha/kit");
        let acorn = acorn(&site);
        assert_eq!(acorn.path, "/alpha/acorn");
        assert_eq!(acorn.topics().next().unwrap().path, "/alpha/acorn/wide");
        let bonus = acorn.topics().find(|t| t.slug == "bonus").unwrap();
        assert_eq!(bonus.pages().next().unwrap().path, "/alpha/acorn/bonus/y1");
        let spare = alpha
            .items()
            .find_map(|item| match item {
                ProductItem::Topic { topic } if topic.slug == "spare" => Some(topic),
                _ => None,
            })
            .unwrap();
        assert_eq!(spare.pages().next().unwrap().path, "/alpha/spare/z1");
    }

    #[test]
    fn loads_topics_and_articles_with_frontmatter() {
        let dir = fixture();
        let site = load(dir.path()).unwrap();

        let acorn = acorn(&site);
        let topics: Vec<_> = acorn.topics().map(|t| t.slug.as_str()).collect();
        assert_eq!(topics, ["wide", "narrow", "bonus"]);

        let wide = acorn.topics().next().unwrap();
        assert_eq!(wide.title, "Wide Topic", "trail.toml overrides the title");
        let narrow = acorn.topics().find(|t| t.slug == "narrow").unwrap();
        assert_eq!(narrow.title, "Narrow", "derived from the slug");
        let articles: Vec<_> = wide.pages().map(|a| a.slug.as_str()).collect();
        assert_eq!(articles, ["a1", "a2", "a3", "a4", "a5"]);
        let a1 = wide.pages().next().unwrap();
        assert_eq!(a1.title, "Article a1");
        assert_eq!(a1.kind.as_deref(), Some("concept"));
        assert_eq!(a1.number, None, "only book articles are numbered");
        assert_eq!(a1.related, ["alpha/b1", "alpha/manual"]);
    }

    #[test]
    fn images_are_collected_with_published_urls() {
        let dir = fixture();
        let site = load(dir.path()).unwrap();

        let urls: Vec<_> = site.images.iter().map(|i| i.url.as_str()).collect();
        assert!(urls.contains(&"/alpha/loose/wiring.png"));
        assert!(urls.contains(&"/alpha/loose/glyph.svg"));
        assert!(urls.contains(&"/alpha/manual/layout.png"));
        let wiring = site
            .images
            .iter()
            .find(|i| i.url == "/alpha/loose/wiring.png")
            .unwrap();
        assert!(wiring.source.ends_with("5--loose.topic/wiring.png"));

        // Articles remember where they loaded from — what their relative
        // image destinations resolve against.
        let loose = alpha(&site)
            .items()
            .find_map(|item| match item {
                ProductItem::Topic { topic } if topic.slug == "loose" => Some(topic),
                _ => None,
            })
            .unwrap();
        let pic = loose.pages().find(|a| a.slug == "pic").unwrap();
        assert!(pic.source_dir.ends_with("alpha.product/5--loose.topic"));
    }

    #[test]
    fn uppercase_image_extensions_are_rejected() {
        let dir = fixture();
        fs::write(
            dir.path().join("alpha.product/5--loose.topic/Shot.PNG"),
            TEST_PNG,
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(
            format!("{err:#}").contains("image 'Shot.PNG' must use a lowercase extension"),
            "got: {err:#}"
        );
    }

    #[test]
    fn loads_topic_subfolders_with_their_own_url_segment() {
        let dir = fixture();
        let site = load(dir.path()).unwrap();

        let narrow = acorn(&site).topics().find(|t| t.slug == "narrow").unwrap();
        let kinds: Vec<_> = narrow
            .children
            .iter()
            .map(|child| match child {
                TopicChild::Article { article } => ("article", article.slug.as_str()),
                TopicChild::Folder { folder } => ("folder", folder.slug.as_str()),
            })
            .collect();
        assert_eq!(
            kinds,
            [("article", "b1"), ("article", "b2"), ("folder", "extra")]
        );

        let TopicChild::Folder { folder } = &narrow.children[2] else {
            panic!("expected the extra folder");
        };
        assert_eq!(folder.title, "Extra", "derived from the slug");
        assert_eq!(
            folder.entry.as_deref(),
            Some("/alpha/acorn/narrow/extra/c1")
        );
        assert_eq!(folder.articles[0].path, "/alpha/acorn/narrow/extra/c1");

        // Flattened reading order feeds the cards; entry is the first page.
        let pages: Vec<_> = narrow.pages().map(|a| a.slug.as_str()).collect();
        assert_eq!(pages, ["b1", "b2", "c1", "linked"]);
        assert_eq!(narrow.entry(), Some("/alpha/acorn/narrow/b1"));

        // The loose topic's empty folder is tolerated and has no entry.
        let loose = alpha(&site)
            .items()
            .find_map(|item| match item {
                ProductItem::Topic { topic } if topic.slug == "loose" => Some(topic),
                _ => None,
            })
            .unwrap();
        let TopicChild::Folder { folder } = loose
            .children
            .iter()
            .find(|c| c.slug() == "hollow")
            .unwrap()
        else {
            panic!("expected the hollow folder");
        };
        assert_eq!(folder.entry, None);
        assert_eq!(folder.title, "Hollow");
    }

    #[test]
    fn rejects_nested_topic_subfolders_and_suffixed_dirs_in_topics() {
        let dir = fixture();
        fs::create_dir_all(
            dir.path()
                .join("alpha.product/2--acorn.antho/200--narrow.topic/5--extra/1--deep"),
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("subfolders hold only articles"));

        let dir = fixture();
        fs::create_dir(
            dir.path()
                .join("alpha.product/2--acorn.antho/200--narrow.topic/6--bad.shelf"),
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("subfolders are plain '<order>--<slug>' directories"));
    }

    #[test]
    fn rejects_slug_collisions_between_articles_and_subfolders() {
        let dir = fixture();
        fs::write(
            dir.path()
                .join("alpha.product/2--acorn.antho/200--narrow.topic/7--extra.md"),
            "---\ntitle: Clash\ntype: concept\ndescription: d\n---\n\nBody.\n",
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("duplicate item slug 'extra'"));
    }

    #[test]
    fn loads_books_with_interleaved_articles_and_chapters() {
        let dir = fixture();
        let site = load(dir.path()).unwrap();

        let manual = alpha(&site).books().find(|b| b.slug == "manual").unwrap();
        assert_eq!(manual.path, "/alpha/manual");
        assert_eq!(manual.title, "Alpha Manual");
        assert_eq!(manual.short.as_deref(), Some("AM"));
        assert_eq!(manual.description, "book description");

        // The body interleaves articles and chapters in one order sequence.
        let kinds: Vec<_> = manual
            .children
            .iter()
            .map(|child| match child {
                BookChild::Article { article } => ("article", article.slug.as_str()),
                BookChild::Chapter { chapter } => ("chapter", chapter.slug.as_str()),
            })
            .collect();
        assert_eq!(
            kinds,
            [
                ("article", "intro"),
                ("chapter", "setup"),
                ("chapter", "appendix"),
                // `a<N>--` appendices sort after every numbered entry,
                // whatever their orders.
                ("article", "glossary"),
                ("chapter", "history"),
            ]
        );

        let BookChild::Article { article: intro } = &manual.children[0] else {
            panic!("expected the intro article");
        };
        assert_eq!(intro.number.as_deref(), Some("1"));
        let BookChild::Chapter { chapter: setup } = &manual.children[1] else {
            panic!("expected the setup chapter");
        };
        assert_eq!(setup.title, "Setup", "derived from the slug");
        assert_eq!(setup.entry, "/alpha/manual/setup/install");
        assert_eq!(setup.number, "2");
        let BookChild::Chapter { chapter: advanced } = &setup.children[1] else {
            panic!("expected the nested advanced chapter");
        };
        assert_eq!(advanced.entry, "/alpha/manual/setup/advanced/tuning");
        assert_eq!(advanced.number, "2.2");
        let BookChild::Article { article: tuning } = &advanced.children[0] else {
            panic!("expected the tuning article");
        };
        assert_eq!(tuning.number.as_deref(), Some("2.2.1"));
        let BookChild::Chapter { chapter: appendix } = &manual.children[2] else {
            panic!("expected the appendix chapter");
        };
        assert_eq!(
            appendix.title, "Appendix A",
            "trail.toml overrides the title"
        );

        // Appendix entries are lettered: a1 → A, a2 → B, children B.1.
        let BookChild::Article { article: glossary } = &manual.children[3] else {
            panic!("expected the glossary appendix");
        };
        assert_eq!(glossary.number.as_deref(), Some("A"));
        // A chapter's own appendix closes out the chapter, lettered
        // within it: setup (2) ends with "2.A".
        let BookChild::Article { article: sidenotes } = setup.children.last().unwrap() else {
            panic!("expected the sidenotes appendix");
        };
        assert_eq!(sidenotes.number.as_deref(), Some("2.A"));
        assert!(sidenotes.appendix);
        let BookChild::Chapter { chapter: history } = &manual.children[4] else {
            panic!("expected the history appendix chapter");
        };
        assert_eq!(history.number, "B");
        let BookChild::Article { article: old } = &history.children[0] else {
            panic!("expected the old article");
        };
        assert_eq!(old.number.as_deref(), Some("B.1"));

        let guide = alpha(&site)
            .books()
            .find(|b| b.slug == "field-guide")
            .unwrap();
        assert_eq!(guide.short, None, "short is optional");
        assert_eq!(guide.path, "/alpha/field-guide", "shelves stay invisible");
    }

    #[test]
    fn appendix_orders_are_books_only_and_validated() {
        // a0 is malformed — appendix orders start at a1.
        let dir = fixture();
        fs::write(
            dir.path().join("alpha.product/7--manual.book/a0--bad.md"),
            "---\ntitle: Bad\n---\n\nBody.\n",
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("appendix orders start at 'a1'"));

        // Topics don't have appendices; the prefix stays digits-only there.
        let dir = fixture();
        fs::write(
            dir.path().join("alpha.product/5--loose.topic/a1--bad.md"),
            "---\ntitle: Bad\ntype: concept\ndescription: d\n---\n\nBody.\n",
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("non-numeric order prefix"));

        // Sharing an appendix order is a collision like any other.
        let dir = fixture();
        fs::create_dir_all(dir.path().join("alpha.product/7--manual.book/a1--dupe")).unwrap();
        fs::write(
            dir.path()
                .join("alpha.product/7--manual.book/a1--dupe/1--x.md"),
            "---\ntitle: X\n---\n\nBody.\n",
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("share order a1"));
    }

    #[test]
    fn links_alias_whole_folders_into_other_topics() {
        let dir = fixture();
        fs::write(
            dir.path()
                .join("alpha.product/5--loose.topic/30--fieldnotes.link"),
            "target = \"~alpha/extra\"\ntitle = \"Field Notes\"\n",
        )
        .unwrap();
        let site = load(dir.path()).unwrap();

        let loose = alpha(&site)
            .items()
            .find_map(|item| match item {
                ProductItem::Topic { topic } if topic.slug == "loose" => Some(topic),
                _ => None,
            })
            .unwrap();
        let folder = loose
            .children
            .iter()
            .find_map(|child| match child {
                TopicChild::Folder { folder } if folder.slug == "fieldnotes" => Some(folder),
                _ => None,
            })
            .expect("the folder alias");
        assert_eq!(folder.title, "Field Notes");
        assert_eq!(folder.path, "/alpha/loose/fieldnotes");
        assert_eq!(folder.entry.as_deref(), Some("/alpha/loose/fieldnotes/c1"));
        // Every article inside is an alias of its original — and an
        // article that was already an alias keeps its true original.
        let c1 = &folder.articles[0];
        assert_eq!(c1.path, "/alpha/loose/fieldnotes/c1");
        assert_eq!(c1.original.as_deref(), Some("/alpha/acorn/narrow/extra/c1"));
        let linked = &folder.articles[1];
        assert_eq!(linked.path, "/alpha/loose/fieldnotes/linked");
        assert_eq!(linked.original.as_deref(), Some("/alpha/loose/x1"));
    }

    #[test]
    fn links_alias_articles_into_books_with_numbers() {
        let dir = fixture();
        let manual = dir.path().join("alpha.product/7--manual.book");
        fs::write(manual.join("5--linked.link"), "target = \"~alpha/x2\"\n").unwrap();
        fs::write(
            manual.join("2--setup/3--sub.link"),
            "target = \"~alpha/x2\"\ntitle = \"Sub X2\"\n",
        )
        .unwrap();
        fs::write(manual.join("a3--applink.link"), "target = \"~alpha/x2\"\n").unwrap();
        // A chapter holding only a link gets its entry at resolution.
        fs::create_dir_all(manual.join("4--linkonly")).unwrap();
        fs::write(
            manual.join("4--linkonly/1--first.link"),
            "target = \"~alpha/x2\"\n",
        )
        .unwrap();
        let site = load(dir.path()).unwrap();

        let manual = alpha(&site).books().find(|b| b.slug == "manual").unwrap();
        let slugs: Vec<_> = manual
            .children
            .iter()
            .map(|child| child.slug().to_string())
            .collect();
        assert_eq!(
            slugs,
            [
                "intro", "setup", "appendix", "linkonly", "linked", "glossary", "history",
                "applink"
            ]
        );
        let BookChild::Article { article: linked } = &manual.children[4] else {
            panic!("expected the linked alias");
        };
        assert_eq!(linked.number.as_deref(), Some("5"));
        assert_eq!(linked.title, "Linked", "derived from the link slug");
        assert_eq!(linked.original.as_deref(), Some("/alpha/loose/x2"));
        assert_eq!(linked.kind, None, "aliases take on book styling");
        assert_eq!(linked.description, None);
        // Appendix links letter like appendix articles: a1, a2, a3 → A, B, C.
        let BookChild::Article { article: applink } = manual.children.last().unwrap() else {
            panic!("expected the applink alias");
        };
        assert_eq!(applink.number.as_deref(), Some("C"));
        assert!(applink.appendix);
        // Chapter-level links number within the chapter.
        let BookChild::Chapter { chapter: setup } = &manual.children[1] else {
            panic!("expected the setup chapter");
        };
        let BookChild::Article { article: sub } = &setup.children[2] else {
            panic!("expected the sub alias");
        };
        assert_eq!(sub.number.as_deref(), Some("2.3"));
        assert_eq!(sub.title, "Sub X2");
        // The link-only chapter's entry lands on its resolved alias.
        let BookChild::Chapter { chapter: linkonly } = &manual.children[3] else {
            panic!("expected the linkonly chapter");
        };
        assert_eq!(linkonly.entry, "/alpha/manual/linkonly/first");
    }

    #[test]
    fn folder_targets_are_rejected_below_topic_level() {
        // Inside a subfolder, only articles can be linked.
        let dir = fixture();
        fs::write(
            dir.path()
                .join("alpha.product/2--acorn.antho/200--narrow.topic/5--extra/9--bad.link"),
            "target = \"~alpha/hollow\"\n",
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(
            format!("{err:#}").contains("a subfolder can only link articles"),
            "got: {err:#}"
        );

        // Books take article links only.
        let dir = fixture();
        fs::write(
            dir.path().join("alpha.product/7--manual.book/5--bad.link"),
            "target = \"~alpha/hollow\"\n",
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(
            format!("{err:#}").contains("only articles can be linked into a book"),
            "got: {err:#}"
        );
    }

    #[test]
    fn theming_config_is_validated() {
        let add_config = |dir: &Path, extra: &str| {
            let config = fs::read_to_string(dir.join("trail.toml")).unwrap();
            fs::write(dir.join("trail.toml"), format!("{extra}\n{config}")).unwrap();
        };

        let dir = fixture();
        add_config(dir.path(), "accent = \"blue\"");
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("not a #rrggbb hex color"));

        let dir = fixture();
        add_config(dir.path(), "accent_dark = \"#112233\"");
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("accent_dark without accent"));

        let dir = fixture();
        add_config(dir.path(), "custom_css = \"custom.css\"");
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("does not exist in the site root"));

        // With the file present it loads, and the root scan tolerates
        // the file that would otherwise be an unexpected entry.
        let dir = fixture();
        add_config(dir.path(), "custom_css = \"custom.css\"");
        fs::write(dir.path().join("custom.css"), "body { color: red }").unwrap();
        let site = load(dir.path()).unwrap();
        assert!(site.custom_css.as_ref().unwrap().ends_with("custom.css"));
    }

    #[test]
    fn passthrough_tolerates_and_validates_root_entries() {
        let add_config = |dir: &Path, extra: &str| {
            let config = fs::read_to_string(dir.join("trail.toml")).unwrap();
            fs::write(dir.join("trail.toml"), format!("{extra}\n{config}")).unwrap();
        };

        // Naming something absent is an error, like favicon and friends.
        let dir = fixture();
        add_config(dir.path(), "passthrough = [\"CNAME\"]");
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("does not exist in the site root"));

        // A passthrough must not reach outside the site to copy.
        let dir = fixture();
        add_config(dir.path(), "passthrough = [\"../secrets\"]");
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("plain relative path"));

        // Nor name the output directory, which it would recurse into.
        let dir = fixture();
        add_config(dir.path(), "passthrough = [\"dist\"]");
        fs::create_dir_all(dir.path().join("dist")).unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("the build output directory"));

        // Present, it loads — and the root scan tolerates both the file
        // and the directory, which would otherwise be unexpected entries.
        let dir = fixture();
        add_config(dir.path(), "passthrough = [\"CNAME\", \"static\"]");
        fs::write(dir.path().join("CNAME"), "learn.example.org\n").unwrap();
        fs::create_dir_all(dir.path().join("static/nested")).unwrap();
        fs::write(dir.path().join("static/nested/robots-extra.txt"), "x").unwrap();
        let site = load(dir.path()).unwrap();
        assert_eq!(site.config.passthrough, ["CNAME", "static"]);
    }

    #[test]
    fn appendix_letters_run_bijectively_past_z() {
        assert_eq!(appendix_letters(1), "A");
        assert_eq!(appendix_letters(26), "Z");
        assert_eq!(appendix_letters(27), "AA");
        assert_eq!(appendix_letters(52), "AZ");
        assert_eq!(appendix_letters(53), "BA");
    }

    #[test]
    fn book_articles_walk_in_reading_order_with_chapter_trails() {
        let dir = fixture();
        let site = load(dir.path()).unwrap();

        let manual = alpha(&site).books().find(|b| b.slug == "manual").unwrap();
        let walked: Vec<_> = manual
            .articles()
            .into_iter()
            .map(|(trail, article)| {
                let trail: Vec<_> = trail.iter().map(|c| c.slug.as_str()).collect();
                (trail, article.slug.as_str())
            })
            .collect();
        assert_eq!(
            walked,
            [
                (vec![], "intro"),
                (vec!["setup"], "install"),
                (vec!["setup", "advanced"], "tuning"),
                (vec!["setup"], "sidenotes"),
                (vec!["appendix"], "tables"),
                (vec![], "glossary"),
                (vec!["history"], "old"),
            ]
        );
    }

    #[test]
    fn rejects_empty_chapters() {
        let dir = fixture();
        fs::create_dir(dir.path().join("alpha.product/7--manual.book/4--hollow")).unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("chapter 'hollow' contains no articles"));
    }

    #[test]
    fn rejects_suffixed_groupings_inside_books() {
        let dir = fixture();
        fs::create_dir(dir.path().join("alpha.product/7--manual.book/4--bad.topic")).unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("chapters are plain '<order>--<slug>' directories"));
    }

    #[test]
    fn rejects_slug_collisions_between_articles_and_chapters() {
        let dir = fixture();
        fs::write(
            dir.path().join("alpha.product/7--manual.book/5--setup.md"),
            "---\ntitle: Clash\n---\n\nBody.\n",
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("duplicate item slug 'setup'"));
    }

    #[test]
    fn rejects_nested_shelves() {
        let dir = fixture();
        fs::create_dir_all(
            dir.path()
                .join("alpha.product/6--tools.shelf/5--inner.shelf"),
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("cannot contain another shelf"));
    }

    #[test]
    fn rejects_url_collisions_across_shelf_boundaries() {
        let dir = fixture();
        // A second "wide" topic inside the anthology's shelf: distinct on
        // disk, same URL space.
        fs::create_dir_all(
            dir.path()
                .join("alpha.product/2--acorn.antho/300--extras.shelf/20--wide.topic"),
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("duplicate item slug 'wide'"));
        assert!(format!("{err:#}").contains("URL space"));
    }

    #[test]
    fn rejects_articles_without_frontmatter() {
        let dir = fixture();
        fs::write(
            dir.path()
                .join("alpha.product/2--acorn.antho/200--narrow.topic/3--bare.md"),
            "no frontmatter here\n",
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("has no frontmatter"));
    }

    #[test]
    fn rejects_grouping_dirs_without_a_type_suffix() {
        let dir = fixture();
        fs::create_dir(dir.path().join("alpha.product/3--stray")).unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("no type suffix"));
    }

    #[test]
    fn links_alias_articles_into_other_locations() {
        let dir = fixture();
        let site = load(dir.path()).unwrap();

        let loose = alpha(&site)
            .items()
            .find_map(|item| match item {
                ProductItem::Topic { topic } if topic.slug == "loose" => Some(topic),
                _ => None,
            })
            .unwrap();
        // The alias slots into reading order under the link's own slug,
        // URL, and derived title, carrying the target's content.
        let pages: Vec<_> = loose.pages().map(|a| a.slug.as_str()).collect();
        assert_eq!(pages, ["x1", "x2", "a1", "alias", "pic"]);
        let alias = loose.pages().find(|a| a.slug == "alias").unwrap();
        assert_eq!(alias.path, "/alpha/loose/alias");
        assert_eq!(alias.title, "Alias", "derived from the link slug");
        assert_eq!(alias.original.as_deref(), Some("/alpha/acorn/wide/a2"));
        assert!(alias.body.contains("Body of a2."));
        assert_eq!(alias.number, None);

        // Folder links work too, with the title override.
        let narrow = acorn(&site).topics().find(|t| t.slug == "narrow").unwrap();
        let linked = narrow.pages().find(|a| a.slug == "linked").unwrap();
        assert_eq!(linked.title, "Linked X1");
        assert_eq!(linked.path, "/alpha/acorn/narrow/extra/linked");
        assert_eq!(linked.original.as_deref(), Some("/alpha/loose/x1"));
    }

    #[test]
    fn reading_times_and_dates_roll_up_the_tree() {
        let dir = fixture();
        let site = load(dir.path()).unwrap();
        let alpha = alpha(&site);

        // An article's own time comes from its word count; frontmatter
        // can override it outright.
        let loose = alpha
            .items()
            .find_map(|item| match item {
                ProductItem::Topic { topic } if topic.slug == "loose" => Some(topic),
                _ => None,
            })
            .unwrap();
        assert!(loose.pages().all(|article| article.reading_minutes >= 1));
        let manual = alpha.books().find(|book| book.slug == "manual").unwrap();
        let intro = manual
            .articles()
            .into_iter()
            .find(|(_, article)| article.slug == "intro")
            .unwrap()
            .1;
        assert_eq!(intro.reading_minutes, 30);
        assert_eq!(intro.updated.as_deref(), Some("2026-05-01"));

        // Containers add their children up...
        assert_eq!(
            loose.reading_minutes,
            loose.pages().map(|a| a.reading_minutes).sum::<u32>()
        );
        assert_eq!(
            manual.reading_minutes,
            manual
                .articles()
                .into_iter()
                .map(|(_, a)| a.reading_minutes)
                .sum::<u32>()
        );
        assert!(manual.reading_minutes >= 30);
        assert_eq!(
            alpha.reading_minutes,
            alpha
                .items()
                .map(|item| match item {
                    ProductItem::Topic { topic } => topic.reading_minutes,
                    ProductItem::Book { book } => book.reading_minutes,
                    ProductItem::Anthology { anthology } => anthology.reading_minutes,
                })
                .sum::<u32>()
        );

        // ...and take the latest date anywhere beneath them.
        assert_eq!(loose.updated.as_deref(), Some("2026-03-15"));
        assert_eq!(manual.updated.as_deref(), Some("2026-05-01"));
        assert_eq!(alpha.updated.as_deref(), Some("2026-05-01"));
        // A topic with no dated articles has no date.
        let narrow_has_dates = alpha
            .anthologies()
            .flat_map(|anthology| anthology.topics())
            .any(|topic| topic.updated.is_some());
        assert!(!narrow_has_dates);
    }

    #[test]
    fn rejects_malformed_updated_dates() {
        for bad in ["March 2026", "2026-3-15", "2026-13-01", "20260315"] {
            let dir = fixture();
            fs::write(
                dir.path().join("alpha.product/5--loose.topic/1--x1.md"),
                format!(
                    "---\ntitle: X\ntype: concept\ndescription: d\nupdated: {bad}\n---\n\nBody.\n"
                ),
            )
            .unwrap();
            // `{:#}` so the assertion sees the whole context chain.
            let error = format!("{:#}", load(dir.path()).unwrap_err());
            assert!(error.contains("not an ISO date"), "{bad} was accepted");
        }
    }

    #[test]
    fn rejects_bad_link_targets() {
        let dir = fixture();
        fs::write(
            dir.path().join("alpha.product/5--loose.topic/6--bad.link"),
            "target = \"~alpha/nope\"\n",
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("resolving link 'bad'"));
        assert!(format!("{err:#}").contains("matches no page"));

        let dir = fixture();
        fs::write(
            dir.path().join("alpha.product/5--loose.topic/6--bad.link"),
            "target = \"~alpha/acorn\"\n",
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(
            format!("{err:#}").contains("is not an article or a topic subfolder"),
            "got: {err:#}"
        );

        let dir = fixture();
        fs::write(
            dir.path().join("alpha.product/5--loose.topic/6--bad.link"),
            "target = \"/alpha/loose/x1\"\n",
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("is not a ~reference"));
    }

    #[test]
    fn loads_books_and_nested_anthologies_inside_anthologies() {
        let dir = fixture();
        let site = load(dir.path()).unwrap();

        let acorn = acorn(&site);
        let items: Vec<_> = acorn.items().map(AnthologyItem::slug).collect();
        assert_eq!(items, ["wide", "narrow", "bonus", "spec", "inner"]);

        let spec = acorn
            .items()
            .find_map(|item| match item {
                AnthologyItem::Book { book } if book.slug == "spec" => Some(book),
                _ => None,
            })
            .unwrap();
        assert_eq!(spec.path, "/alpha/acorn/spec");
        let (_, rules) = spec.articles().into_iter().next().unwrap();
        assert_eq!(rules.path, "/alpha/acorn/spec/rules");
        assert_eq!(rules.number.as_deref(), Some("1"));
        assert_eq!(rules.kind, None, "book frontmatter is title-only");
        assert_eq!(rules.description, None);

        let inner = acorn
            .items()
            .find_map(|item| match item {
                AnthologyItem::Anthology { anthology } if anthology.slug == "inner" => {
                    Some(anthology)
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(inner.path, "/alpha/acorn/inner");
        let deep = inner.topics().next().unwrap();
        assert_eq!(
            deep.pages().next().unwrap().path,
            "/alpha/acorn/inner/deep/d1"
        );
    }

    #[test]
    fn rejects_taxonomy_frontmatter_in_book_articles() {
        let dir = fixture();
        fs::write(
            dir.path().join("alpha.product/7--manual.book/6--stray.md"),
            "---\ntitle: Stray\ntype: concept\ndescription: d\n---\n\nBody.\n",
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("unknown field"));
    }

    #[test]
    fn rejects_duplicate_slugs_across_grouping_kinds() {
        let dir = fixture();
        let clash = dir.path().join("alpha.product/8--acorn.topic");
        fs::create_dir(&clash).unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("duplicate grouping slug 'acorn'"));
    }

    #[test]
    fn derives_topic_titles_by_capitalising_slug_words() {
        assert_eq!(title_from_slug("identity"), "Identity");
        assert_eq!(title_from_slug("the-two-gates"), "The Two Gates");
        assert_eq!(
            title_from_slug("well-known-principals"),
            "Well Known Principals"
        );
    }

    #[test]
    fn rejects_topic_descriptions() {
        let dir = fixture();
        fs::write(
            dir.path()
                .join("alpha.product/2--acorn.antho/200--narrow.topic/trail.toml"),
            "title = \"Narrow\"\ndescription = \"not allowed\"\n",
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("description"));
    }

    #[test]
    fn rejects_duplicate_anthology_orders() {
        let dir = fixture();
        let dup = dir.path().join("alpha.product/2--dup.antho");
        fs::create_dir(&dup).unwrap();
        fs::write(
            dup.join("trail.toml"),
            "title = \"D\"\ndescription = \"d\"\n",
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("share order 2"));
    }

    #[test]
    fn rejects_reserved_slugs() {
        let dir = fixture();
        fs::create_dir(dir.path().join("alpha.product/12--print.topic")).unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("'print' is a reserved slug"));

        let dir = fixture();
        let assets = dir.path().join("assets.product");
        fs::create_dir(&assets).unwrap();
        fs::write(
            assets.join("trail.toml"),
            "title = \"A\"\nmonogram = \"Aa\"\ncolor = \"#3b82f6\"\ndescription = \"d\"\n",
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("reserved product slug"));
    }

    #[test]
    fn rejects_unexpected_root_entries() {
        let dir = fixture();
        fs::write(dir.path().join("stray.txt"), "").unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(err.to_string().contains("unexpected file 'stray.txt'"));
    }

    #[test]
    fn rejects_unknown_featured_slugs() {
        let dir = fixture();
        let config = fs::read_to_string(dir.path().join("trail.toml")).unwrap();
        fs::write(
            dir.path().join("trail.toml"),
            config.replace("\"beta\"", "\"nope\""),
        )
        .unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(err.to_string().contains("featured product 'nope'"));
    }

    #[test]
    fn rejects_bad_colors() {
        let dir = fixture();
        let toml_path = dir.path().join("alpha.product/trail.toml");
        let config = fs::read_to_string(&toml_path).unwrap();
        fs::write(&toml_path, config.replace("#3b82f6", "blue")).unwrap();
        let err = load(dir.path()).unwrap_err();
        assert!(format!("{err:#}").contains("loading product 'alpha'"));
    }

    #[test]
    fn tolerates_the_out_dir_inside_the_root() {
        let dir = fixture();
        fs::create_dir(dir.path().join("dist")).unwrap();
        load(dir.path()).unwrap();
    }
}
