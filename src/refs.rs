//! Inline references: phrases that link to their declaring page wherever
//! prose states them, and the `§` section-number index behind them.
//!
//! Declaration is self-service — a thing claims the phrases that mean it:
//! an article in its frontmatter (`inline_ref:`), a book, chapter, topic,
//! subfolder, anthology or product in its trail.toml (`inline_ref = []`).
//! A phrase links to the declarer's page, or its first article where no
//! page exists (chapters, topics, subfolders). A phrase claimed by a book
//! may carry a `§<number>` suffix in prose to reach one section; bare
//! `§<number>` references resolve within the surrounding book.

use std::collections::HashMap;

use aho_corasick::{AhoCorasick, AhoCorasickBuilder, MatchKind};
use anyhow::{Result, bail, ensure};

use crate::markdown;
use crate::site::{
    Anthology, AnthologyItem, Article, Book, BookChild, ProductItem, Site, Topic, TopicChild,
};

/// Where a phrase points.
#[derive(Debug)]
pub enum PhraseTarget {
    /// A page path: the declaring article, or a directory's landing.
    Page(String),
    /// A book's cover path; prose may extend the phrase with `§<number>`.
    Book(String),
}

/// One declared phrase and its target.
#[derive(Debug)]
pub struct Phrase {
    pub text: String,
    pub target: PhraseTarget,
}

/// A `§` target inside a book: an article page, optionally one heading.
#[derive(Debug)]
pub struct SectionTarget {
    pub path: String,
    pub fragment: Option<String>,
}

/// One `[*name]` citation anchor, resolved against the tree.
#[derive(Debug, Clone)]
pub struct Citation {
    /// The declared name, unique within its book.
    pub name: String,
    /// The full citation string a test or code comment writes, e.g.
    /// `Kernel TRM *copy-up.preserves-ownership`. Falls back to the
    /// book's title where it declares no short name.
    pub qualified: String,
    /// Site-absolute path of the book's cover.
    pub book: String,
    /// Site-absolute path of the article the anchor sits in.
    pub article: String,
    /// The article's section number ("4.6") where it has one.
    pub number: Option<String>,
    /// The source block the anchor sits in. Block context, not the
    /// statement — an anchor is a locator and delimits nothing.
    pub context: String,
    /// True when the anchor names a whole section rather than one
    /// statement inside it.
    pub section: bool,
}

#[derive(Debug, Default)]
pub struct InlineRefIndex {
    /// Leftmost-longest matcher over every phrase; None when no phrase
    /// is declared anywhere.
    matcher: Option<AhoCorasick>,
    /// Phrase details, indexed by the matcher's pattern ids.
    phrases: Vec<Phrase>,
    /// Per-book section index: book path → section number → target.
    /// Built for every book, phrases declared or not — bare `§`
    /// references work in any book.
    sections: HashMap<String, HashMap<String, SectionTarget>>,
    /// Every citation anchor in the site, in tree order.
    citations: Vec<Citation>,
}

impl InlineRefIndex {
    pub fn new(site: &Site) -> Result<InlineRefIndex> {
        let mut builder = Builder::default();
        for product in &site.products {
            builder.claim(
                &product.inline_refs,
                || PhraseTarget::Page(product.path.clone()),
                &format!("product '{}'", product.path),
            )?;
            for item in product.items() {
                match item {
                    ProductItem::Topic { topic } => builder.topic(topic)?,
                    ProductItem::Book { book } => builder.book(book)?,
                    ProductItem::Anthology { anthology } => builder.anthology(anthology)?,
                }
            }
        }
        builder.finish()
    }

    pub fn matcher(&self) -> Option<&AhoCorasick> {
        self.matcher.as_ref()
    }

    /// The phrase behind one of the matcher's pattern ids.
    pub fn phrase(&self, pattern: usize) -> &Phrase {
        &self.phrases[pattern]
    }

    /// Look up a section number ("2.1", "A.3") within a book.
    pub fn section(&self, book: &str, number: &str) -> Option<&SectionTarget> {
        self.sections.get(book)?.get(number)
    }

    /// Every citation anchor in the site, in tree order.
    pub fn citations(&self) -> &[Citation] {
        &self.citations
    }
}

#[derive(Default)]
struct Builder {
    phrases: Vec<Phrase>,
    /// phrase → declarer, for duplicate reports.
    claimed: HashMap<String, String>,
    sections: HashMap<String, HashMap<String, SectionTarget>>,
    citations: Vec<Citation>,
}

impl Builder {
    fn claim<F: Fn() -> PhraseTarget>(
        &mut self,
        phrases: &[String],
        target: F,
        declarer: &str,
    ) -> Result<()> {
        for phrase in phrases {
            ensure!(
                !phrase.trim().is_empty(),
                "{declarer} declares an empty inline_ref phrase"
            );
            ensure!(
                phrase.trim() == phrase,
                "{declarer} declares inline_ref phrase '{phrase}' with \
                 leading or trailing whitespace"
            );
            ensure!(
                !phrase.contains('§'),
                "{declarer} declares inline_ref phrase '{phrase}' containing '§'"
            );
            if let Some(other) = self.claimed.insert(phrase.clone(), declarer.to_string()) {
                bail!("inline_ref phrase '{phrase}' is claimed by both {other} and {declarer}");
            }
            self.phrases.push(Phrase {
                text: phrase.clone(),
                target: target(),
            });
        }
        Ok(())
    }

    fn anthology(&mut self, anthology: &Anthology) -> Result<()> {
        self.claim(
            &anthology.inline_refs,
            || PhraseTarget::Page(anthology.path.clone()),
            &format!("anthology '{}'", anthology.path),
        )?;
        for item in anthology.items() {
            match item {
                AnthologyItem::Topic { topic } => self.topic(topic)?,
                AnthologyItem::Book { book } => self.book(book)?,
                AnthologyItem::Anthology { anthology } => self.anthology(anthology)?,
            }
        }
        Ok(())
    }

    fn topic(&mut self, topic: &Topic) -> Result<()> {
        if !topic.inline_refs.is_empty() {
            let Some(entry) = topic.entry() else {
                bail!(
                    "topic '{}' declares inline_ref but holds no articles to land on",
                    topic.path
                );
            };
            let entry = entry.to_string();
            self.claim(
                &topic.inline_refs,
                || PhraseTarget::Page(entry.clone()),
                &format!("topic '{}'", topic.path),
            )?;
        }
        for child in &topic.children {
            match child {
                TopicChild::Article { article } => {
                    Self::reject_citations(article)?;
                    self.article(article)?;
                }
                TopicChild::Folder { folder } => {
                    if !folder.inline_refs.is_empty() {
                        let Some(entry) = &folder.entry else {
                            bail!(
                                "subfolder '{}' declares inline_ref but holds \
                                 no articles to land on",
                                folder.path
                            );
                        };
                        self.claim(
                            &folder.inline_refs,
                            || PhraseTarget::Page(entry.clone()),
                            &format!("subfolder '{}'", folder.path),
                        )?;
                    }
                    for article in &folder.articles {
                        Self::reject_citations(article)?;
                        self.article(article)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Citation anchors are addressed as "<book> *<name>", so a book is
    /// the only thing that can carry one. An anchor in a task-doc topic
    /// would be unreachable and silently uncovered — fail instead.
    fn reject_citations(article: &Article) -> Result<()> {
        if let Some(anchor) = markdown::citation_anchors(&article.body).first() {
            bail!(
                "article '{}' declares citation '*{}', but citation anchors \
                 are only valid inside a book",
                article.path,
                anchor.name
            );
        }
        Ok(())
    }

    fn article(&mut self, article: &Article) -> Result<()> {
        if article.original.is_some() {
            // Aliases never claim; the original already did.
            return Ok(());
        }
        self.claim(
            &article.inline_refs,
            || PhraseTarget::Page(article.path.clone()),
            &format!("article '{}'", article.path),
        )
    }

    fn book(&mut self, book: &Book) -> Result<()> {
        self.claim(
            &book.inline_refs,
            || PhraseTarget::Book(book.path.clone()),
            &format!("book '{}'", book.path),
        )?;
        self.book_children(&book.children)?;

        // The section index. Article and chapter numbers come from the
        // loaded tree; heading numbers ("2.1.3") from the same outline
        // pass the renderer uses, so ids can never drift apart.
        let mut sections: HashMap<String, SectionTarget> = HashMap::new();
        for (_, article) in book.articles() {
            let Some(number) = &article.number else {
                continue;
            };
            sections.insert(
                number.clone(),
                SectionTarget {
                    path: article.path.clone(),
                    fragment: None,
                },
            );
            for entry in markdown::heading_outline(&article.body, Some(number)) {
                if let Some(number) = entry.number {
                    sections.insert(
                        number,
                        SectionTarget {
                            path: article.path.clone(),
                            fragment: Some(entry.id),
                        },
                    );
                }
            }
        }
        fn chapter_numbers(children: &[BookChild], sections: &mut HashMap<String, SectionTarget>) {
            for child in children {
                if let BookChild::Chapter { chapter } = child {
                    sections.insert(
                        chapter.number.clone(),
                        SectionTarget {
                            path: chapter.entry.clone(),
                            fragment: None,
                        },
                    );
                    chapter_numbers(&chapter.children, sections);
                }
            }
        }
        chapter_numbers(&book.children, &mut sections);
        self.sections.insert(book.path.clone(), sections);
        self.book_citations(book)?;
        Ok(())
    }

    /// Collect the book's `[*name]` anchors, enforcing that a name is
    /// unique within its book. Uniqueness is per book rather than
    /// site-wide (as inline_ref phrases are): names like
    /// `copy-up.preserves-ownership` are naturally book-scoped, and a
    /// citation from outside the corpus is qualified by the book's
    /// short name anyway.
    fn book_citations(&mut self, book: &Book) -> Result<()> {
        let short = book.short.clone().unwrap_or_else(|| book.title.clone());
        let mut seen: HashMap<String, String> = HashMap::new();
        for (_, article) in book.articles() {
            if article.original.is_some() {
                // Aliases re-present another article; its anchors are
                // already collected at the original.
                continue;
            }
            for anchor in markdown::citation_anchors(&article.body) {
                if let Some(other) = seen.insert(anchor.name.clone(), article.path.clone()) {
                    bail!(
                        "citation '*{}' is declared twice in book '{}': \
                         '{other}' and '{}'",
                        anchor.name,
                        book.path,
                        article.path
                    );
                }
                self.citations.push(Citation {
                    qualified: format!("{short} *{}", anchor.name),
                    name: anchor.name,
                    book: book.path.clone(),
                    article: article.path.clone(),
                    number: article.number.clone(),
                    context: anchor.context,
                    section: anchor.section,
                });
            }
        }
        Ok(())
    }

    fn book_children(&mut self, children: &[BookChild]) -> Result<()> {
        for child in children {
            match child {
                BookChild::Article { article } => self.article(article)?,
                BookChild::Chapter { chapter } => {
                    if !chapter.inline_refs.is_empty() {
                        self.claim(
                            &chapter.inline_refs,
                            || PhraseTarget::Page(chapter.entry.clone()),
                            &format!("chapter '{}'", chapter.path),
                        )?;
                    }
                    self.book_children(&chapter.children)?;
                }
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<InlineRefIndex> {
        let matcher = if self.phrases.is_empty() {
            None
        } else {
            Some(
                AhoCorasickBuilder::new()
                    .match_kind(MatchKind::LeftmostLongest)
                    .build(self.phrases.iter().map(|phrase| phrase.text.as_str()))?,
            )
        };
        Ok(InlineRefIndex {
            matcher,
            phrases: self.phrases,
            sections: self.sections,
            citations: self.citations,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_index() -> (tempfile::TempDir, InlineRefIndex) {
        let dir = tempfile::tempdir().unwrap();
        crate::site::write_fixture(dir.path());
        let site = Site::load(dir.path(), &dir.path().join("dist")).unwrap();
        let index = InlineRefIndex::new(&site).unwrap();
        (dir, index)
    }

    #[test]
    fn section_index_covers_articles_chapters_headings_and_appendices() {
        let (_dir, index) = fixture_index();
        let section = |number: &str| index.section("/alpha/manual", number).unwrap();
        // Articles and chapters by their numbers; a chapter lands on its
        // first article.
        assert_eq!(section("1").path, "/alpha/manual/intro");
        assert_eq!(section("2").path, "/alpha/manual/setup/install");
        assert_eq!(section("2.1").path, "/alpha/manual/setup/install");
        assert_eq!(section("2.2.1").path, "/alpha/manual/setup/advanced/tuning");
        // Headings resolve to their article plus fragment.
        let heading = section("2.1.1");
        assert_eq!(heading.path, "/alpha/manual/setup/install");
        assert_eq!(
            heading.fragment.as_deref(),
            Some("first-section-of-install")
        );
        // Appendix letters work at every level.
        assert_eq!(section("A").path, "/alpha/manual/glossary");
        assert_eq!(section("2.A").path, "/alpha/manual/setup/sidenotes");
        assert_eq!(section("B.1").path, "/alpha/manual/history/old");
        assert!(index.section("/alpha/manual", "9.9").is_none());
        // Books without declared phrases still get a section index.
        assert!(index.section("/alpha/field-guide", "1").is_some());
    }

    #[test]
    fn phrases_resolve_to_their_declarers() {
        let (_dir, index) = fixture_index();
        let matcher = index.matcher().unwrap();
        let hit = matcher.find("see the Alpha Manual here").unwrap();
        match &index.phrase(hit.pattern().as_usize()).target {
            PhraseTarget::Book(path) => assert_eq!(path, "/alpha/manual"),
            other => panic!("expected a book target, got {other:?}"),
        }
        let hit = matcher.find("X-Two explains").unwrap();
        match &index.phrase(hit.pattern().as_usize()).target {
            PhraseTarget::Page(path) => assert_eq!(path, "/alpha/loose/x2"),
            other => panic!("expected a page target, got {other:?}"),
        }
    }

    #[test]
    fn citations_carry_their_book_article_and_context() {
        let (_dir, index) = fixture_index();
        let citation = index
            .citations()
            .iter()
            .find(|c| c.name == "install.preserves-ownership")
            .expect("fixture anchor");
        // Qualified with the book's short name — the string a test writes.
        assert_eq!(citation.qualified, "AM *install.preserves-ownership");
        assert_eq!(citation.book, "/alpha/manual");
        assert_eq!(citation.article, "/alpha/manual/setup/install");
        assert_eq!(citation.number.as_deref(), Some("2.1"));
        // Block context, with the marker gone and the code sample's
        // lookalike never having been an anchor at all.
        assert!(citation.context.starts_with("The installer preserves ownership."));
        // The real marker is stripped; the inline-code lookalike in the
        // same block is left exactly as written, because it was never an
        // anchor to begin with.
        assert!(!citation.context.contains("[*install.preserves-ownership]"));
        assert!(citation.context.contains("`[*not.an.anchor]`"));
        assert!(index.citations().iter().all(|c| c.name != "not.an.anchor"));
    }

    #[test]
    fn a_name_declared_twice_in_one_book_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        crate::site::write_fixture(dir.path());
        // A second declaration of the install article's name, from
        // another article of the same book.
        let path = dir
            .path()
            .join("alpha.product/7--manual.book/1--intro.md");
        let body = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, body + "\nAgain. [*install.preserves-ownership]\n").unwrap();
        let site = Site::load(dir.path(), &dir.path().join("dist")).unwrap();
        let err = InlineRefIndex::new(&site).unwrap_err();
        assert!(err.to_string().contains("declared twice"), "{err}");
        assert!(err.to_string().contains("install.preserves-ownership"), "{err}");
    }

    #[test]
    fn a_citation_outside_a_book_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        crate::site::write_fixture(dir.path());
        // A topic article: nothing could ever cite it, since a citation
        // is addressed through its book.
        let path = dir
            .path()
            .join("alpha.product/2--acorn.antho/100--wide.topic/1--a1.md");
        let body = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, body + "\nStatement. [*loose.anchor]\n").unwrap();
        let site = Site::load(dir.path(), &dir.path().join("dist")).unwrap();
        let err = InlineRefIndex::new(&site).unwrap_err();
        assert!(err.to_string().contains("only valid inside a book"), "{err}");
    }

    #[test]
    fn duplicate_phrases_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        crate::site::write_fixture(dir.path());
        // A second claim of "X-Two", from another article's frontmatter.
        std::fs::write(
            dir.path()
                .join("alpha.product/2--acorn.antho/100--wide.topic/1--a1.md"),
            "---\ntitle: Article a1\ntype: concept\ndescription: about a1\n\
             inline_ref:\n  - X-Two\n---\n\nBody of a1.\n",
        )
        .unwrap();
        let site = Site::load(dir.path(), &dir.path().join("dist")).unwrap();
        let err = InlineRefIndex::new(&site).unwrap_err();
        assert!(err.to_string().contains("claimed by both"));
        assert!(err.to_string().contains("X-Two"));
    }
}
