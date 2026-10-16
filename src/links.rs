use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::site::{Anthology, AnthologyItem, Book, ProductItem, Site, Topic, TopicChild};

/// Why a reference failed to resolve. Dangling kinds (unknown product,
/// no match) can be downgraded to warnings; ambiguity never is.
#[derive(Debug)]
pub enum ResolveError {
    UnknownProduct(String),
    NoMatch(String),
    Ambiguous(String),
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::UnknownProduct(message)
            | ResolveError::NoMatch(message)
            | ResolveError::Ambiguous(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for ResolveError {}

/// Resolves `~` cross-references ("~peios/identity/sids") to concrete page
/// paths. The first segment names a product; the rest is matched as a
/// suffix of the URL-path segments of that product's pages (anthology pages
/// and article pages). Exactly one match resolves; zero or several are
/// errors — the shortest unambiguous suffix is always enough, and deeper
/// segments (including the anthology's) exist only for disambiguation.
#[derive(Debug, Default)]
pub struct LinkIndex {
    products: HashMap<String, ProductIndex>,
    /// Page path → the anchor ids its body defines (heading ids, `[*name]`
    /// citation anchors, raw HTML ids), so a `#fragment` on a reference
    /// can be checked. Only articles have bodies; product and anthology
    /// pages are absent and their fragments go unchecked.
    anchors: HashMap<String, HashSet<String>>,
}

#[derive(Debug)]
struct ProductIndex {
    path: String,
    title: String,
    candidates: Vec<Candidate>,
    // Candidate indices retain traversal order, including ambiguous matches.
    suffixes: HashMap<String, Vec<usize>>,
}

#[derive(Debug)]
struct Candidate {
    /// URL-path segments below the product, e.g. ["acorn", "wide", "a1"].
    segments: Vec<String>,
    /// The page the candidate resolves to.
    target: String,
    /// The page's display title, for surfaces that show the link
    /// (related-content lists).
    title: String,
}

/// Add a topic's articles as candidates: [<anthologies..>,] topic,
/// [folder,] article — every segment, folders included, exists both as
/// an address and for disambiguation. With `folders` set the subfolders
/// themselves become endpoints too — used only when resolving `.link`
/// stubs, which may alias a whole folder; body links never target one.
fn push_topic_candidates(
    candidates: &mut Vec<Candidate>,
    prefix: &[String],
    topic: &Topic,
    folders: bool,
) {
    let segments = |tail: &[&str]| -> Vec<String> {
        prefix
            .iter()
            .cloned()
            .chain(std::iter::once(topic.slug.clone()))
            .chain(tail.iter().map(|s| s.to_string()))
            .collect()
    };
    for child in &topic.children {
        match child {
            TopicChild::Article { article } => candidates.push(Candidate {
                segments: segments(&[&article.slug]),
                target: article.path.clone(),
                title: article.title.clone(),
            }),
            TopicChild::Folder { folder } => {
                if folders {
                    candidates.push(Candidate {
                        segments: segments(&[&folder.slug]),
                        target: folder.path.clone(),
                        title: folder.title.clone(),
                    });
                }
                for article in &folder.articles {
                    candidates.push(Candidate {
                        segments: segments(&[&folder.slug, &article.slug]),
                        target: article.path.clone(),
                        title: article.title.clone(),
                    });
                }
            }
        }
    }
}

/// A book's cover and its articles: [<anthologies..>,] book,
/// [chapters..,] article.
fn push_book_candidates(candidates: &mut Vec<Candidate>, prefix: &[String], book: &Book) {
    let mut cover: Vec<String> = prefix.to_vec();
    cover.push(book.slug.clone());
    candidates.push(Candidate {
        segments: cover.clone(),
        target: book.path.clone(),
        title: book.title.clone(),
    });
    for (chapters, article) in book.articles() {
        let mut segments = cover.clone();
        segments.extend(chapters.iter().map(|c| c.slug.clone()));
        segments.push(article.slug.clone());
        candidates.push(Candidate {
            segments,
            target: article.path.clone(),
            title: article.title.clone(),
        });
    }
}

/// An anthology's page and everything beneath it — anthologies nest, so
/// this recurses with a growing prefix.
fn push_anthology_candidates(
    candidates: &mut Vec<Candidate>,
    prefix: &[String],
    anthology: &Anthology,
    folders: bool,
) {
    let mut base: Vec<String> = prefix.to_vec();
    base.push(anthology.slug.clone());
    candidates.push(Candidate {
        segments: base.clone(),
        target: anthology.path.clone(),
        title: anthology.title.clone(),
    });
    for item in anthology.items() {
        match item {
            AnthologyItem::Topic { topic } => {
                push_topic_candidates(candidates, &base, topic, folders)
            }
            AnthologyItem::Book { book } => push_book_candidates(candidates, &base, book),
            AnthologyItem::Anthology { anthology } => {
                push_anthology_candidates(candidates, &base, anthology, folders)
            }
        }
    }
}

impl LinkIndex {
    pub fn new(site: &Site) -> LinkIndex {
        Self::build(site, false)
    }

    /// Like `new`, but topic subfolders are endpoints too. Only `.link`
    /// stub resolution uses this — a stub may alias a whole folder.
    pub(crate) fn with_folders(site: &Site) -> LinkIndex {
        Self::build(site, true)
    }

    fn build(site: &Site, folders: bool) -> LinkIndex {
        let mut products = HashMap::new();
        for product in &site.products {
            let mut candidates = Vec::new();
            for item in product.items() {
                match item {
                    ProductItem::Anthology { anthology } => {
                        push_anthology_candidates(&mut candidates, &[], anthology, folders);
                    }
                    ProductItem::Topic { topic } => {
                        push_topic_candidates(&mut candidates, &[], topic, folders);
                    }
                    ProductItem::Book { book } => {
                        push_book_candidates(&mut candidates, &[], book);
                    }
                }
            }
            let mut suffixes: HashMap<String, Vec<usize>> = HashMap::new();
            for (i, candidate) in candidates.iter().enumerate() {
                for start in 0..candidate.segments.len() {
                    suffixes.entry(candidate.segments[start..].join("/")).or_default().push(i);
                }
            }
            products.insert(
                product.slug.clone(),
                ProductIndex {
                    path: product.path.clone(),
                    title: product.title.clone(),
                    candidates,
                    suffixes,
                },
            );
        }
        let anchors = site
            .articles()
            .into_iter()
            .map(|article| {
                (
                    article.path.clone(),
                    crate::markdown::anchor_ids(&article.body),
                )
            })
            .collect();
        LinkIndex { products, anchors }
    }

    /// Whether the page at `path` defines the anchor `fragment`. None when
    /// the page's anchors are unknown — a product or anthology page, or an
    /// index built without a site — and the caller should not judge.
    pub fn has_anchor(&self, path: &str, fragment: &str) -> Option<bool> {
        self.anchors.get(path).map(|ids| ids.contains(fragment))
    }

    /// Resolve a reference with its leading `~` already stripped.
    pub fn resolve(&self, reference: &str) -> Result<String, ResolveError> {
        self.resolve_page(reference).map(|(path, _)| path)
    }

    /// Resolve like `resolve`, also returning the target page's title —
    /// for surfaces that display the link rather than embed it.
    pub fn resolve_page(&self, reference: &str) -> Result<(String, String), ResolveError> {
        let mut segments = reference.split('/');
        let product_slug = segments.next().unwrap_or("");
        let suffix: Vec<&str> = segments.collect();

        let Some(product) = self.products.get(product_slug) else {
            return Err(ResolveError::UnknownProduct(format!(
                "link '~{reference}' names unknown product '{product_slug}'"
            )));
        };
        if suffix.is_empty() {
            return Ok((product.path.clone(), product.title.clone()));
        }

        let key = suffix.join("/");
        let matches: Vec<&Candidate> = product.suffixes.get(&key).into_iter().flatten()
            .map(|i| &product.candidates[*i]).collect();

        match matches.as_slice() {
            [only] => Ok((only.target.clone(), only.title.clone())),
            [] => Err(ResolveError::NoMatch(format!(
                "link '~{reference}' matches no page in product '{product_slug}'"
            ))),
            several => Err(ResolveError::Ambiguous(format!(
                "link '~{reference}' is ambiguous; candidates:\n{}",
                several
                    .iter()
                    .map(|c| format!("  ~{}/{}", product_slug, c.segments.join("/")))
                    .collect::<Vec<_>>()
                    .join("\n")
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site;

    #[test]
    fn indexed_suffixes_match_the_scan_including_ambiguity_order() {
        let (_dir, index) = index();
        for product in index.products.values() {
            let mut queries = HashSet::from(["missing".to_owned(), "".to_owned()]);
            for candidate in &product.candidates {
                for start in 0..candidate.segments.len() {
                    let suffix = candidate.segments[start..].join("/");
                    queries.insert(format!("{suffix}/"));
                    queries.insert(format!("/{suffix}"));
                    queries.insert(suffix);
                }
            }
            for query in queries {
                let suffix: Vec<_> = query.split('/').collect();
                let expected: Vec<_> = product.candidates.iter().enumerate()
                    .filter(|(_, c)| c.segments.len() >= suffix.len()
                        && c.segments[c.segments.len() - suffix.len()..]
                            .iter().map(String::as_str).eq(suffix.iter().copied()))
                    .map(|(i, _)| i).collect();
                let actual = product.suffixes.get(&query).cloned().unwrap_or_default();
                assert_eq!(actual, expected, "suffix {query:?}");
            }
        }
    }

    fn index() -> (tempfile::TempDir, LinkIndex) {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        let site = Site::load(dir.path(), &dir.path().join("dist")).unwrap();
        let index = LinkIndex::new(&site);
        (dir, index)
    }

    #[test]
    fn resolves_the_shortest_unambiguous_suffix() {
        let (_dir, index) = index();
        assert_eq!(index.resolve("alpha/a2").unwrap(), "/alpha/acorn/wide/a2");
        assert_eq!(index.resolve("alpha/x1").unwrap(), "/alpha/loose/x1");
    }

    #[test]
    fn anchors_are_known_for_articles_only() {
        let (_dir, index) = index();
        assert_eq!(
            index.has_anchor("/alpha/acorn/wide/a2", "first-section-of-a2"),
            Some(true)
        );
        assert_eq!(index.has_anchor("/alpha/acorn/wide/a2", "nope"), Some(false));
        // Product and anthology pages have no body to define anchors in.
        assert_eq!(index.has_anchor("/alpha", "anything"), None);
        assert_eq!(index.has_anchor("/alpha/acorn", "anything"), None);
    }

    #[test]
    fn longer_suffixes_disambiguate_including_anthology_segments() {
        let (_dir, index) = index();
        // "a1" exists in both the wide (anthology) topic and the loose (bare)
        // topic, so the short form is ambiguous...
        let err = index.resolve("alpha/a1").unwrap_err().to_string();
        assert!(err.contains("ambiguous"));
        assert!(err.contains("~alpha/acorn/wide/a1"));
        assert!(err.contains("~alpha/loose/a1"));
        // ...and either topic or anthology segments settle it.
        assert_eq!(
            index.resolve("alpha/wide/a1").unwrap(),
            "/alpha/acorn/wide/a1"
        );
        assert_eq!(index.resolve("alpha/loose/a1").unwrap(), "/alpha/loose/a1");
        assert_eq!(
            index.resolve("alpha/acorn/wide/a1").unwrap(),
            "/alpha/acorn/wide/a1"
        );
    }

    #[test]
    fn resolves_anthology_and_product_pages() {
        let (_dir, index) = index();
        assert_eq!(index.resolve("alpha/acorn").unwrap(), "/alpha/acorn");
        assert_eq!(index.resolve("alpha").unwrap(), "/alpha");
    }

    #[test]
    fn resolves_book_covers_and_articles() {
        let (_dir, index) = index();
        // A bare book reference lands on the cover page.
        assert_eq!(index.resolve("alpha/manual").unwrap(), "/alpha/manual");
        // Chapter segments are addresses like any other; the shortest
        // unambiguous suffix reaches even deeply nested articles.
        assert_eq!(
            index.resolve("alpha/tuning").unwrap(),
            "/alpha/manual/setup/advanced/tuning"
        );
        assert_eq!(
            index.resolve("alpha/advanced/tuning").unwrap(),
            "/alpha/manual/setup/advanced/tuning"
        );
        assert_eq!(
            index.resolve("alpha/manual/setup/install").unwrap(),
            "/alpha/manual/setup/install"
        );
        assert_eq!(
            index.resolve("alpha/notes").unwrap(),
            "/alpha/field-guide/notes"
        );
    }

    #[test]
    fn resolves_topic_subfolder_articles() {
        let (_dir, index) = index();
        assert_eq!(
            index.resolve("alpha/c1").unwrap(),
            "/alpha/acorn/narrow/extra/c1"
        );
        assert_eq!(
            index.resolve("alpha/extra/c1").unwrap(),
            "/alpha/acorn/narrow/extra/c1"
        );
        assert_eq!(
            index.resolve("alpha/narrow/extra/c1").unwrap(),
            "/alpha/acorn/narrow/extra/c1"
        );
    }

    #[test]
    fn resolves_books_and_anthologies_nested_in_anthologies() {
        let (_dir, index) = index();
        // A book inside an anthology, addressable at any suffix depth.
        assert_eq!(index.resolve("alpha/spec").unwrap(), "/alpha/acorn/spec");
        assert_eq!(
            index.resolve("alpha/rules").unwrap(),
            "/alpha/acorn/spec/rules"
        );
        assert_eq!(
            index.resolve("alpha/acorn/spec/rules").unwrap(),
            "/alpha/acorn/spec/rules"
        );
        // A nested anthology and its contents.
        assert_eq!(index.resolve("alpha/inner").unwrap(), "/alpha/acorn/inner");
        assert_eq!(
            index.resolve("alpha/d1").unwrap(),
            "/alpha/acorn/inner/deep/d1"
        );
    }

    #[test]
    fn resolve_page_carries_display_titles() {
        let (_dir, index) = index();
        let page = |reference: &str| index.resolve_page(reference).unwrap();
        assert_eq!(
            page("alpha/a2"),
            ("/alpha/acorn/wide/a2".into(), "Article a2".into())
        );
        assert_eq!(
            page("alpha/manual"),
            ("/alpha/manual".into(), "Alpha Manual".into())
        );
        assert_eq!(
            page("alpha/acorn"),
            ("/alpha/acorn".into(), "Acorn Docs".into())
        );
        assert_eq!(page("alpha"), ("/alpha".into(), "Alpha".into()));
    }

    #[test]
    fn unknown_targets_and_products_are_errors() {
        let (_dir, index) = index();
        let err = index.resolve("alpha/nope").unwrap_err().to_string();
        assert!(err.contains("matches no page in product 'alpha'"));
        let err = index.resolve("ghost/x").unwrap_err().to_string();
        assert!(err.contains("unknown product 'ghost'"));
    }
}
