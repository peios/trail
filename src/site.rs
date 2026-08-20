use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};

/// The site model: everything the renderer needs, loaded and validated.
#[derive(Debug)]
pub struct Site {
    pub config: SiteConfig,
    /// All products in display order: featured first (in `featured` order),
    /// then the rest sorted by title.
    pub products: Vec<Product>,
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
    #[serde(default = "default_true")]
    pub built_by_trail: bool,
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
}

#[derive(Debug, Serialize)]
pub struct Anthology {
    pub slug: String,
    /// Site-absolute URL path, e.g. "/peios/security-fundamentals".
    pub path: String,
    pub order: u32,
    pub title: String,
    pub description: String,
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
}

/// A topic's, shelf's or chapter's *optional* `trail.toml`: nothing but an
/// explicit title, for casing the derived default can't produce (acronyms
/// and such).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TitleConfig {
    title: String,
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
        let mut s = serializer.serialize_struct("Topic", 7)?;
        s.serialize_field("slug", &self.slug)?;
        s.serialize_field("path", &self.path)?;
        s.serialize_field("order", &self.order)?;
        s.serialize_field("title", &self.title)?;
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
#[derive(Debug, Serialize)]
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
    /// authored `NN--` orders along its chapter trail. None outside books.
    pub number: Option<String>,
    /// The page taxonomy label from frontmatter (`type:`), e.g. "concept".
    /// None inside books, where the taxonomy is meaningless.
    pub kind: Option<String>,
    /// None inside books; sections of a formal document aren't summarised
    /// individually.
    pub description: Option<String>,
    /// Unresolved cross-reference slugs from frontmatter; resolution comes
    /// with the linking layer.
    pub related: Vec<String>,
    /// For a page created by a `.link` reference: the path of the
    /// canonical article whose content this page re-renders. Linked
    /// pages stay out of the search index — the original covers it.
    pub original: Option<String>,
    /// The markdown body after the frontmatter. Not exposed to templates —
    /// it is rendered separately and passed to the article page as HTML.
    #[serde(skip)]
    pub body: String,
}

/// An article's YAML frontmatter. Unknown keys are load errors.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArticleConfig {
    title: String,
    #[serde(rename = "type")]
    kind: String,
    description: String,
    #[serde(default)]
    related: Vec<String>,
}

/// A book article's YAML frontmatter: just a title — the learn taxonomy
/// (`type:`) and per-page descriptions are meaningless inside a formal
/// document, so providing them is an error.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BookArticleConfig {
    title: String,
    #[serde(default)]
    related: Vec<String>,
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
    /// Direct children in `NN--` order. Serialized: templates walk the
    /// tree for the cover's contents and the article sidebar.
    pub children: Vec<BookChild>,
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
    pub number: String,
    /// Derived from the slug unless the chapter's trail.toml overrides it.
    pub title: String,
    /// URL path of the chapter's first article in reading order — where
    /// chapter links (contents, breadcrumbs, sidebar) land.
    pub entry: String,
    pub children: Vec<BookChild>,
}

/// A book's `trail.toml`. Unknown keys are load errors.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BookConfig {
    title: String,
    short: Option<String>,
    description: String,
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
#[derive(Debug)]
pub struct LinkStub {
    slug: String,
    order: u32,
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

        let mut products = Vec::new();
        for entry in read_dir_sorted(root)? {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || is_same_path(&entry.path(), out) {
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
                    load_product(&path, slug)
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

        let mut site = Site { config, products };
        resolve_linked_articles(&mut site)?;
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
}

fn load_product(dir: &Path, slug: &str) -> Result<Product> {
    let config: ProductConfig = read_toml(&dir.join("trail.toml"))?;
    validate_color(&config.color)?;
    let path = format!("/{slug}");

    let mut children = Vec::new();
    for (child_dir, name) in grouping_dirs(dir, "a product")? {
        let grouping = parse_grouping_name(&name)?;
        let child = match grouping.kind.as_str() {
            "antho" => ProductChild::Item(ProductItem::Anthology {
                anthology: load_anthology(&child_dir, grouping, &path)
                    .with_context(|| format!("loading anthology in '{name}'"))?,
            }),
            "topic" => ProductChild::Item(ProductItem::Topic {
                topic: load_topic(&child_dir, grouping, &path)
                    .with_context(|| format!("loading topic in '{name}'"))?,
            }),
            "book" => ProductChild::Item(ProductItem::Book {
                book: load_book(&child_dir, grouping, &path)
                    .with_context(|| format!("loading book in '{name}'"))?,
            }),
            "shelf" => ProductChild::Shelf(
                load_product_shelf(&child_dir, grouping, &path)
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
        children,
    };
    check_flat_slugs("grouping", product.items().map(ProductItem::slug))?;
    Ok(product)
}

fn load_product_shelf(
    dir: &Path,
    name: GroupingName,
    product_path: &str,
) -> Result<Shelf<ProductItem>> {
    let title = grouping_title(dir, &name.slug)?;

    let mut items = Vec::new();
    for (child_dir, entry_name) in grouping_dirs(dir, "a shelf")? {
        let grouping = parse_grouping_name(&entry_name)?;
        let item = match grouping.kind.as_str() {
            "antho" => ProductItem::Anthology {
                anthology: load_anthology(&child_dir, grouping, product_path)
                    .with_context(|| format!("loading anthology in '{entry_name}'"))?,
            },
            "topic" => ProductItem::Topic {
                topic: load_topic(&child_dir, grouping, product_path)
                    .with_context(|| format!("loading topic in '{entry_name}'"))?,
            },
            "book" => ProductItem::Book {
                book: load_book(&child_dir, grouping, product_path)
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

fn load_anthology(dir: &Path, name: GroupingName, parent_path: &str) -> Result<Anthology> {
    let config: AnthologyConfig = read_toml(&dir.join("trail.toml"))?;
    let path = format!("{parent_path}/{}", name.slug);

    let mut children = Vec::new();
    for (child_dir, entry_name) in grouping_dirs(dir, "an anthology")? {
        let grouping = parse_grouping_name(&entry_name)?;
        let child = match grouping.kind.as_str() {
            "antho" => AnthologyChild::Item(AnthologyItem::Anthology {
                anthology: load_anthology(&child_dir, grouping, &path)
                    .with_context(|| format!("loading anthology in '{entry_name}'"))?,
            }),
            "topic" => AnthologyChild::Item(AnthologyItem::Topic {
                topic: load_topic(&child_dir, grouping, &path)
                    .with_context(|| format!("loading topic in '{entry_name}'"))?,
            }),
            "book" => AnthologyChild::Item(AnthologyItem::Book {
                book: load_book(&child_dir, grouping, &path)
                    .with_context(|| format!("loading book in '{entry_name}'"))?,
            }),
            "shelf" => AnthologyChild::Shelf(
                load_anthology_shelf(&child_dir, grouping, &path)
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
        children,
    };
    check_flat_slugs("item", anthology.items().map(AnthologyItem::slug))?;
    Ok(anthology)
}

fn load_anthology_shelf(
    dir: &Path,
    name: GroupingName,
    anthology_path: &str,
) -> Result<Shelf<AnthologyItem>> {
    let title = grouping_title(dir, &name.slug)?;

    let mut items = Vec::new();
    for (child_dir, entry_name) in grouping_dirs(dir, "a shelf")? {
        let grouping = parse_grouping_name(&entry_name)?;
        let item = match grouping.kind.as_str() {
            "antho" => AnthologyItem::Anthology {
                anthology: load_anthology(&child_dir, grouping, anthology_path)
                    .with_context(|| format!("loading anthology in '{entry_name}'"))?,
            },
            "topic" => AnthologyItem::Topic {
                topic: load_topic(&child_dir, grouping, anthology_path)
                    .with_context(|| format!("loading topic in '{entry_name}'"))?,
            },
            "book" => AnthologyItem::Book {
                book: load_book(&child_dir, grouping, anthology_path)
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

fn load_topic(dir: &Path, name: GroupingName, parent_path: &str) -> Result<Topic> {
    let title = grouping_title(dir, &name.slug)?;
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
                folder: load_topic_folder(&entry.path(), order, slug, &path)
                    .with_context(|| format!("loading subfolder in '{entry_name}'"))?,
            });
        } else if let Some(stem) = entry_name.strip_suffix(".link") {
            links.push(load_link_stub(&entry.path(), stem)?);
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
        title,
        children,
        links,
    })
}

fn load_topic_folder(dir: &Path, order: u32, slug: &str, parent_path: &str) -> Result<TopicFolder> {
    let title = grouping_title(dir, slug)?;
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
        if let Some(stem) = entry_name.strip_suffix(".link") {
            links.push(load_link_stub(&entry.path(), stem)?);
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
        title,
        entry: articles.first().map(|article| article.path.clone()),
        articles,
        links,
    })
}

/// Parse a `<order>--<slug>.link` reference file.
fn load_link_stub(file: &Path, stem: &str) -> Result<LinkStub> {
    let Some((order, slug)) = stem.split_once("--") else {
        bail!("link '{stem}.link' is missing its '<order>--' prefix");
    };
    let order: u32 = order
        .parse()
        .with_context(|| format!("link '{stem}.link' has a non-numeric order prefix"))?;
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
        title: config.title.unwrap_or_else(|| title_from_slug(slug)),
        target: config.target,
    })
}

fn load_book(dir: &Path, name: GroupingName, parent_path: &str) -> Result<Book> {
    let config: BookConfig = read_toml(&dir.join("trail.toml"))?;
    let path = format!("{parent_path}/{}", name.slug);
    let children = load_book_children(dir, &path, "", "a book")?;

    Ok(Book {
        slug: name.slug,
        path,
        order: name.order,
        title: config.title,
        short: config.short,
        description: config.description,
        children,
    })
}

fn load_chapter(
    dir: &Path,
    order: u32,
    slug: &str,
    parent_path: &str,
    number: String,
) -> Result<Chapter> {
    let title = grouping_title(dir, slug)?;
    let path = format!("{parent_path}/{slug}");
    let children = load_book_children(dir, &path, &format!("{number}."), "a chapter")?;

    // Chapter links open the chapter's first article, so a chapter with
    // nothing to open is a mistake, not an empty page.
    let entry = match children.first() {
        Some(BookChild::Article { article }) => article.path.clone(),
        Some(BookChild::Chapter { chapter }) => chapter.entry.clone(),
        None => bail!("chapter '{slug}' contains no articles"),
    };

    Ok(Chapter {
        slug: slug.to_string(),
        path,
        order,
        number,
        title,
        entry,
        children,
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
) -> Result<Vec<BookChild>> {
    let mut children = Vec::new();
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
            let Some((order, slug)) = entry_name.split_once("--") else {
                bail!("chapter '{entry_name}' is missing its '<order>--' prefix");
            };
            let order: u32 = order.parse().with_context(|| {
                format!("chapter '{entry_name}' has a non-numeric order prefix")
            })?;
            validate_slug(slug).with_context(|| format!("in chapter '{entry_name}'"))?;
            let number = format!("{number_prefix}{order}");
            children.push(BookChild::Chapter {
                chapter: load_chapter(&entry.path(), order, slug, parent_path, number)
                    .with_context(|| format!("loading chapter in '{entry_name}'"))?,
            });
        } else {
            let Some(stem) = entry_name.strip_suffix(".md") else {
                bail!("unexpected file '{entry_name}' in {what}: articles are *.md files");
            };
            children.push(BookChild::Article {
                article: load_article(&entry.path(), stem, parent_path, Some(number_prefix))?,
            });
        }
    }
    children.sort_by(|a, b| {
        a.order()
            .cmp(&b.order())
            .then_with(|| a.slug().cmp(b.slug()))
    });
    // Articles and chapters share the book's URL space, so slugs are
    // checked across both.
    check_duplicates("item", &children, BookChild::order, BookChild::slug)?;
    Ok(children)
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
    let Some((order, slug)) = stem.split_once("--") else {
        bail!("article '{stem}.md' is missing its '<order>--' prefix");
    };
    let order: u32 = order
        .parse()
        .with_context(|| format!("article '{stem}.md' has a non-numeric order prefix"))?;
    validate_slug(slug).with_context(|| format!("in article '{stem}.md'"))?;

    let context = || format!("loading article '{slug}'");
    let (title, kind, description, related, body) = match number_prefix {
        Some(_) => {
            let (frontmatter, body): (BookArticleConfig, _) =
                read_article(file).with_context(context)?;
            (frontmatter.title, None, None, frontmatter.related, body)
        }
        None => {
            let (frontmatter, body): (ArticleConfig, _) =
                read_article(file).with_context(context)?;
            (
                frontmatter.title,
                Some(frontmatter.kind),
                Some(frontmatter.description),
                frontmatter.related,
                body,
            )
        }
    };
    Ok(Article {
        slug: slug.to_string(),
        path: format!("{parent_path}/{slug}"),
        order,
        number: number_prefix.map(|prefix| format!("{prefix}{order}")),
        title,
        kind,
        description,
        related,
        original: None,
        body,
    })
}

/// Resolve every `.link` stub into a real article: the target's content
/// under the link's own slug, title, URL, and chrome. Runs after the
/// whole tree has loaded, since targets can live anywhere; targets must
/// be articles that exist on disk — a link cannot target another link.
fn resolve_linked_articles(site: &mut Site) -> Result<()> {
    let index = crate::links::LinkIndex::new(site);
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

    let resolve = |stub: LinkStub, parent_path: &str| -> Result<Article> {
        let reference = stub.target.trim_start_matches('~');
        let resolved = index
            .resolve(reference)
            .with_context(|| format!("resolving link '{}' in '{parent_path}'", stub.slug))?;
        let Some(original) = originals.get(&resolved) else {
            bail!(
                "link '{}' in '{parent_path}' targets '{}', which is not an article \
                 (only articles can be linked, and a link cannot target another link)",
                stub.slug,
                stub.target
            );
        };
        let mut article = original.clone();
        article.slug = stub.slug.clone();
        article.path = format!("{parent_path}/{}", stub.slug);
        article.order = stub.order;
        article.title = stub.title;
        article.number = None;
        article.original = Some(resolved);
        Ok(article)
    };

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

    for topic in topics_mut(&mut site.products) {
        for stub in std::mem::take(&mut topic.links) {
            let article = resolve(stub, &topic.path)?;
            topic.children.push(TopicChild::Article { article });
        }
        topic.children.sort_by(|a, b| {
            a.order()
                .cmp(&b.order())
                .then_with(|| a.slug().cmp(b.slug()))
        });
        check_duplicates("item", &topic.children, TopicChild::order, TopicChild::slug)
            .with_context(|| format!("in topic '{}'", topic.path))?;
        for child in &mut topic.children {
            let TopicChild::Folder { folder } = child else {
                continue;
            };
            if folder.links.is_empty() {
                continue;
            }
            for stub in std::mem::take(&mut folder.links) {
                folder.articles.push(resolve(stub, &folder.path)?);
            }
            folder
                .articles
                .sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.slug.cmp(&b.slug)));
            check_duplicates("article", &folder.articles, |a| a.order, |a| &a.slug)
                .with_context(|| format!("in subfolder '{}'", folder.path))?;
            folder.entry = folder.articles.first().map(|article| article.path.clone());
        }
    }
    Ok(())
}

/// A topic's, shelf's or chapter's title: derived from the slug unless an
/// optional trail.toml overrides it.
fn grouping_title(dir: &Path, slug: &str) -> Result<String> {
    let config_path = dir.join("trail.toml");
    if config_path.exists() {
        let config: TitleConfig = read_toml(&config_path)?;
        Ok(config.title)
    } else {
        Ok(title_from_slug(slug))
    }
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
fn check_duplicates<T>(
    what: &str,
    items: &[T],
    order: impl Fn(&T) -> u32,
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

fn is_same_path(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

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
                    "---\ntitle: Article {slug}\ntype: concept\ndescription: about {slug}\nrelated:\n  - somewhere/else\n---\n\nBody of {slug}.\n\n## First section of {slug}\n\nWords.\n"
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
        "title = \"Alpha Manual\"\nshort = \"AM\"\ndescription = \"book description\"\n",
    )
    .unwrap();
    fs::write(manual.join("1--intro.md"), article("intro")).unwrap();
    fs::write(manual.join("2--setup/1--install.md"), article("install")).unwrap();
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
        assert_eq!(a1.related, ["somewhere/else"]);
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
                ("chapter", "appendix")
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

        let guide = alpha(&site)
            .books()
            .find(|b| b.slug == "field-guide")
            .unwrap();
        assert_eq!(guide.short, None, "short is optional");
        assert_eq!(guide.path, "/alpha/field-guide", "shelves stay invisible");
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
                (vec!["appendix"], "tables"),
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
        assert_eq!(pages, ["x1", "x2", "a1", "alias"]);
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
        assert!(format!("{err:#}").contains("only articles can be linked"));

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
