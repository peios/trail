use anyhow::{Context, Result};
use minijinja::{Environment, context};

use crate::markdown;
use crate::site::{Anthology, Article, Book, Chapter, Product, Site, Topic, TopicFolder};

const TEMPLATES: &[(&str, &str)] = &[
    ("base.html", include_str!("../theme/base.html")),
    ("index.html", include_str!("../theme/index.html")),
    ("product.html", include_str!("../theme/product.html")),
    ("cards.html", include_str!("../theme/cards.html")),
    ("anthology.html", include_str!("../theme/anthology.html")),
    ("article.html", include_str!("../theme/article.html")),
    ("book.html", include_str!("../theme/book.html")),
    (
        "book-article.html",
        include_str!("../theme/book-article.html"),
    ),
    ("book-nav.html", include_str!("../theme/book-nav.html")),
    ("print.html", include_str!("../theme/print.html")),
];

/// One entry in an article's Related Content list: a resolved
/// `related:` reference, ready to display.
#[derive(Debug, serde::Serialize)]
pub struct RelatedLink {
    pub path: String,
    pub title: String,
}

/// One article's slot on a `/print` single-page view.
#[derive(Debug, serde::Serialize)]
pub struct PrintSection {
    /// Anchor id, unique across the whole print page.
    pub id: String,
    /// The article's section number inside a book, if any.
    pub number: Option<String>,
    pub title: String,
    /// Display crumb segments orienting the article within the unit;
    /// the template joins them (an interpolated "/" would be escaped).
    pub crumbs: Vec<String>,
    /// The rendered article body.
    pub html: String,
}

pub struct Renderer {
    env: Environment<'static>,
    /// Inject the dev-server auto-reload script into every page.
    live_reload: bool,
}

impl Renderer {
    pub fn new(live_reload: bool) -> Result<Renderer> {
        let mut env = Environment::new();
        for (name, source) in TEMPLATES {
            env.add_template(name, source)
                .with_context(|| format!("parsing template {name}"))?;
        }
        Ok(Renderer { env, live_reload })
    }

    pub fn index(&self, site: &Site) -> Result<String> {
        let (head, tail) = split_sitename(&site.config.sitename);
        self.env
            .get_template("index.html")
            .expect("template registered in new()")
            .render(context! {
                site => site.config,
                products => site.products,
                featured => site.featured(),
                md_path => "/llms.txt",
                wordmark_head => head,
                wordmark_tail => tail,
                live_reload => self.live_reload,
            })
            .context("rendering index.html")
    }

    pub fn product(&self, site: &Site, product: &Product) -> Result<String> {
        let (head, tail) = split_sitename(&site.config.sitename);
        self.env
            .get_template("product.html")
            .expect("template registered in new()")
            .render(context! {
                site => site.config,
                products => site.products,
                product => product,
                sections => product.sections(),
                md_path => format!("{}.md", product.path),
                wordmark_head => head,
                wordmark_tail => tail,
                live_reload => self.live_reload,
            })
            .with_context(|| format!("rendering product page '{}'", product.slug))
    }

    pub fn anthology(
        &self,
        site: &Site,
        product: &Product,
        parents: &[&Anthology],
        anthology: &Anthology,
    ) -> Result<String> {
        let (head, tail) = split_sitename(&site.config.sitename);
        self.env
            .get_template("anthology.html")
            .expect("template registered in new()")
            .render(context! {
                site => site.config,
                products => site.products,
                product => product,
                anthology => anthology,
                anthologies => parents,
                sections => anthology.sections(),
                md_path => format!("{}.md", anthology.path),
                wordmark_head => head,
                wordmark_tail => tail,
                live_reload => self.live_reload,
            })
            .with_context(|| {
                format!(
                    "rendering anthology page '{}/{}'",
                    product.slug, anthology.slug
                )
            })
    }

    pub fn book(
        &self,
        site: &Site,
        product: &Product,
        anthologies: &[&Anthology],
        book: &Book,
    ) -> Result<String> {
        let (head, tail) = split_sitename(&site.config.sitename);
        self.env
            .get_template("book.html")
            .expect("template registered in new()")
            .render(context! {
                site => site.config,
                products => site.products,
                product => product,
                anthologies => anthologies,
                book => book,
                md_path => format!("{}.md", book.path),
                wordmark_head => head,
                wordmark_tail => tail,
                live_reload => self.live_reload,
            })
            .with_context(|| format!("rendering book cover '{}/{}'", product.slug, book.slug))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn book_article(
        &self,
        site: &Site,
        product: &Product,
        anthologies: &[&Anthology],
        book: &Book,
        chapters: &[&Chapter],
        article: &Article,
        rendered: &markdown::Rendered,
        related: &[RelatedLink],
    ) -> Result<String> {
        let (head, tail) = split_sitename(&site.config.sitename);
        self.env
            .get_template("book-article.html")
            .expect("template registered in new()")
            .render(context! {
                site => site.config,
                products => site.products,
                product => product,
                anthologies => anthologies,
                book => book,
                chapters => chapters,
                article => article,
                related => related,
                md_path => format!("{}.md", article.path),
                content => rendered.html,
                toc => rendered.toc,
                has_mermaid => rendered.has_mermaid,
                wordmark_head => head,
                wordmark_tail => tail,
                live_reload => self.live_reload,
            })
            .with_context(|| format!("rendering article page '{}'", article.path))
    }

    /// A unit's single-page view: every article in reading order.
    #[allow(clippy::too_many_arguments)]
    pub fn print(
        &self,
        site: &Site,
        product: &Product,
        title: &str,
        description: Option<&str>,
        md_path: &str,
        sections: &[PrintSection],
        has_mermaid: bool,
    ) -> Result<String> {
        let (head, tail) = split_sitename(&site.config.sitename);
        self.env
            .get_template("print.html")
            .expect("template registered in new()")
            .render(context! {
                site => site.config,
                products => site.products,
                product => product,
                title => title,
                description => description,
                md_path => md_path,
                sections => sections,
                has_mermaid => has_mermaid,
                wordmark_head => head,
                wordmark_tail => tail,
                live_reload => self.live_reload,
            })
            .with_context(|| format!("rendering print page for '{title}'"))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn article(
        &self,
        site: &Site,
        product: &Product,
        anthologies: &[&Anthology],
        topic: &Topic,
        folder: Option<&TopicFolder>,
        article: &Article,
        rendered: &markdown::Rendered,
        related: &[RelatedLink],
    ) -> Result<String> {
        let (head, tail) = split_sitename(&site.config.sitename);
        self.env
            .get_template("article.html")
            .expect("template registered in new()")
            .render(context! {
                site => site.config,
                products => site.products,
                product => product,
                anthologies => anthologies,
                topic => topic,
                folder => folder,
                article => article,
                related => related,
                md_path => format!("{}.md", article.path),
                content => rendered.html,
                toc => rendered.toc,
                has_mermaid => rendered.has_mermaid,
                wordmark_head => head,
                wordmark_tail => tail,
                live_reload => self.live_reload,
            })
            .with_context(|| format!("rendering article page '{}'", article.path))
    }
}

/// The wordmark accents the last word of the sitename ("Peios <Learn>");
/// a single-word sitename gets no accent.
fn split_sitename(name: &str) -> (&str, Option<&str>) {
    match name.rsplit_once(' ') {
        Some((head, tail)) => (head, Some(tail)),
        None => (name, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::site::{self, Site};

    fn fixture_site() -> (tempfile::TempDir, Site) {
        let dir = tempfile::tempdir().unwrap();
        site::write_fixture(dir.path());
        let site = Site::load(dir.path(), &dir.path().join("dist")).unwrap();
        (dir, site)
    }

    #[test]
    fn renders_site_and_product_data_on_the_front_page() {
        let (_dir, site) = fixture_site();
        let html = Renderer::new(false).unwrap().index(&site).unwrap();

        assert!(html.contains("<title>Test Learn</title>"));
        assert!(html.contains("Docs for testing"));
        assert!(html.contains("Alpha"));
        assert!(html.contains("--pc: #3b82f6"));
        assert!(
            !html.contains("{{"),
            "unrendered template syntax left in output"
        );
    }

    #[test]
    fn renders_anthology_cards_on_the_product_page() {
        let (_dir, site) = fixture_site();
        let alpha = site.products.iter().find(|p| p.slug == "alpha").unwrap();
        let html = Renderer::new(false).unwrap().product(&site, alpha).unwrap();

        assert!(html.contains("<title>Alpha — Test Learn</title>"));
        assert!(html.contains("Acorn Docs"));
        assert!(html.contains("href=\"/alpha/zulu\""));
        // The bare topic renders as a topic card, articles linked directly.
        assert!(html.contains("href=\"/alpha/loose/x1\">Loose"));
        assert!(html.contains("href=\"/alpha/loose/x2\">Article x2"));
        assert!(
            !html.contains("{{"),
            "unrendered template syntax left in output"
        );
    }

    #[test]
    fn anthology_page_lists_topics_with_the_see_all_threshold() {
        let (_dir, site) = fixture_site();
        let alpha = site.products.iter().find(|p| p.slug == "alpha").unwrap();
        let acorn = alpha.anthologies().find(|d| d.slug == "acorn").unwrap();
        let html = Renderer::new(false)
            .unwrap()
            .anthology(&site, alpha, &[], acorn)
            .unwrap();

        // Wide topic: 5 articles → first 3 plus a see-all button.
        assert!(html.contains("Article a1"));
        assert!(html.contains("Article a3"));
        assert!(!html.contains("Article a4"));
        // The see-all button opens the topic's first article.
        assert!(html.contains("href=\"/alpha/acorn/wide/a1\">See all 5 articles"));
        // Topic titles also open the first article; no bare topic URLs remain.
        assert!(html.contains("href=\"/alpha/acorn/wide/a1\">Wide Topic"));
        assert!(!html.contains("href=\"/alpha/acorn/wide\""));
        // Narrow topic: 2 articles → all listed, no button.
        assert!(html.contains("Article b2"));
        assert!(!html.contains("See all 2"));
        assert!(html.contains("href=\"/alpha/acorn/narrow/b1\""));
    }

    #[test]
    fn book_cover_is_a_standard_page_with_the_sidebar_tree() {
        let (_dir, site) = fixture_site();
        let alpha = site.products.iter().find(|p| p.slug == "alpha").unwrap();
        let manual = alpha.books().find(|b| b.slug == "manual").unwrap();
        let html = Renderer::new(false)
            .unwrap()
            .book(&site, alpha, &[], manual)
            .unwrap();

        assert!(html.contains("<title>Alpha Manual — Test Learn</title>"));
        assert!(html.contains("book description"));
        // The cover sits in the standard page chrome; the sidebar tree is
        // the contents, so the body carries no second copy.
        assert!(html.contains("article-layout"));
        assert!(html.contains("aria-current=\"page\">AM</a>"));
        assert!(!html.contains(">Contents<"));
        // The sidebar holds articles and chapters at every depth, numbered
        // from their NN-- orders, chapters linking to their first article.
        assert!(html.contains(
            "href=\"/alpha/manual/intro\"><span class=\"section-number\">1</span>Article intro"
        ));
        assert!(html.contains(
            "href=\"/alpha/manual/setup/install\"><span class=\"section-number\">2</span>Setup"
        ));
        assert!(html.contains(">2.2</span>Advanced"));
        assert!(html.contains(">3</span>Appendix A</a>"));
        // Nothing is current on the cover, so every chapter starts collapsed.
        assert_eq!(html.matches("<details open>").count(), 0);
        assert!(
            !html.contains("{{"),
            "unrendered template syntax left in output"
        );
    }

    #[test]
    fn book_article_page_has_tree_sidebar_and_chapter_crumbs() {
        let (_dir, site) = fixture_site();
        let alpha = site.products.iter().find(|p| p.slug == "alpha").unwrap();
        let manual = alpha.books().find(|b| b.slug == "manual").unwrap();
        let (chapters, article) = manual
            .articles()
            .into_iter()
            .find(|(_, a)| a.slug == "tuning")
            .unwrap();
        let rendered = crate::markdown::render(
            &article.body,
            &crate::links::LinkIndex::default(),
            crate::markdown::RenderOptions {
                numbering: article.number.as_deref(),
                ..crate::markdown::RenderOptions::default()
            },
        )
        .unwrap();
        let html = Renderer::new(false)
            .unwrap()
            .book_article(
                &site,
                alpha,
                &[],
                manual,
                &chapters,
                article,
                &rendered,
                &[],
            )
            .unwrap();

        // The sidebar shows the whole book, current article marked and
        // numbered; the short name heads it and links to the cover.
        assert!(html.contains(
            "aria-current=\"page\"><span class=\"section-number\">2.2.1</span>Article tuning</a>"
        ));
        assert!(html.contains("href=\"/alpha/manual\">AM</a>"));
        assert!(html.contains("href=\"/alpha/manual/intro\""));
        assert!(html.contains("href=\"/alpha/manual/appendix/tables\""));
        // Chapters on the trail to the current page are expanded; the
        // appendix stays collapsed.
        assert_eq!(html.matches("<details open>").count(), 2);
        assert_eq!(html.matches("<details>").count(), 1);
        // Breadcrumbs walk the chapter trail.
        assert!(html.contains("href=\"/alpha/manual/setup/install\">Setup</a>"));
        assert!(html.contains("href=\"/alpha/manual/setup/advanced/tuning\">Advanced</a>"));
        assert!(html.contains("Body of tuning."));
        // The page title and its headings carry the section numbering,
        // and the ToC mirrors it.
        assert!(
            html.contains("<h1><span class=\"heading-number\">2.2.1</span> Article tuning</h1>")
        );
        assert!(
            html.contains("<span class=\"heading-number\">2.2.1.1</span> First section of tuning")
        );
        assert!(
            html.contains("<span class=\"section-number\">2.2.1.1</span>First section of tuning")
        );
    }

    #[test]
    fn article_page_has_sidebar_content_and_toc() {
        let (_dir, site) = fixture_site();
        let alpha = site.products.iter().find(|p| p.slug == "alpha").unwrap();
        let acorn = alpha.anthologies().find(|d| d.slug == "acorn").unwrap();
        let wide = acorn.topics().next().unwrap();
        let article = wide.pages().nth(1).unwrap();
        let rendered = crate::markdown::render(
            &article.body,
            &crate::links::LinkIndex::default(),
            crate::markdown::RenderOptions::default(),
        )
        .unwrap();
        let html = Renderer::new(false)
            .unwrap()
            .article(&site, alpha, &[acorn], wide, None, article, &rendered, &[])
            .unwrap();

        // Sidebar lists all the topic's articles, marking the current one.
        assert!(html.contains("aria-current=\"page\">Article a2</a>"));
        assert!(html.contains("href=\"/alpha/acorn/wide/a5\""));
        // Rendered markdown body with heading ids.
        assert!(html.contains("Body of a2."));
        assert!(html.contains("<h2 id=\"first-section-of-a2\">"));
        // ToC links to the heading, and the scroll-spy script rides along.
        assert!(html.contains("href=\"#first-section-of-a2\""));
        assert!(html.contains("Scroll-spy"));
        // Breadcrumb trail.
        assert!(html.contains(">Acorn Docs</a>"));
        // No diagram → no mermaid script.
        assert!(!html.contains("mermaid.min.js"));
    }

    #[test]
    fn articles_with_diagrams_pull_in_the_mermaid_script() {
        let (_dir, site) = fixture_site();
        let alpha = site.products.iter().find(|p| p.slug == "alpha").unwrap();
        let acorn = alpha.anthologies().find(|d| d.slug == "acorn").unwrap();
        let wide = acorn.topics().next().unwrap();
        let article = wide.pages().next().unwrap();
        let rendered = crate::markdown::render(
            "```mermaid\ngraph TD;\n  A-->B;\n```\n",
            &crate::links::LinkIndex::default(),
            crate::markdown::RenderOptions::default(),
        )
        .unwrap();
        let html = Renderer::new(false)
            .unwrap()
            .article(&site, alpha, &[acorn], wide, None, article, &rendered, &[])
            .unwrap();

        assert!(html.contains("<pre class=\"mermaid\">"));
        assert!(html.contains("src=\"/assets/mermaid.min.js\""));
        assert!(html.contains("mermaid.initialize"));
    }

    #[test]
    fn topic_subfolders_render_as_sidebar_groups_with_crumb_segments() {
        let (_dir, site) = fixture_site();
        let alpha = site.products.iter().find(|p| p.slug == "alpha").unwrap();
        let acorn = alpha.anthologies().find(|d| d.slug == "acorn").unwrap();
        let narrow = acorn.topics().find(|t| t.slug == "narrow").unwrap();
        let crate::site::TopicChild::Folder { folder } = &narrow.children[2] else {
            panic!("expected the extra folder");
        };
        let article = &folder.articles[0];
        let rendered = crate::markdown::render(
            &article.body,
            &crate::links::LinkIndex::default(),
            crate::markdown::RenderOptions::default(),
        )
        .unwrap();
        let html = Renderer::new(false)
            .unwrap()
            .article(
                &site,
                alpha,
                &[acorn],
                narrow,
                Some(folder),
                article,
                &rendered,
                &[],
            )
            .unwrap();

        // Sidebar: the folder is a collapsible group, open because the
        // current article is inside it, with the article marked.
        assert!(html.contains("<details open>"));
        assert!(html.contains("aria-current=\"page\">Article c1</a>"));
        assert!(html.contains(">Extra</a></summary>"));
        // Crumbs gain the folder segment; search metadata mirrors it.
        assert!(html.contains("href=\"/alpha/acorn/narrow/extra/c1\">Extra</a>"));
        assert!(html.contains("data-pagefind-meta=\"crumbs:Alpha / Acorn Docs / Narrow / Extra\""));

        // A direct-article page shows the group collapsed.
        let b1 = narrow.pages().next().unwrap();
        let rendered = crate::markdown::render(
            &b1.body,
            &crate::links::LinkIndex::default(),
            crate::markdown::RenderOptions::default(),
        )
        .unwrap();
        let html = Renderer::new(false)
            .unwrap()
            .article(&site, alpha, &[acorn], narrow, None, b1, &rendered, &[])
            .unwrap();
        assert!(html.contains("<details>"));
        assert!(!html.contains("<details open>"));
    }

    #[test]
    fn bare_topic_article_breadcrumb_has_no_anthology_segment() {
        let (_dir, site) = fixture_site();
        let alpha = site.products.iter().find(|p| p.slug == "alpha").unwrap();
        let loose = alpha
            .items()
            .find_map(|item| match item {
                crate::site::ProductItem::Topic { topic } if topic.slug == "loose" => Some(topic),
                _ => None,
            })
            .unwrap();
        let article = loose.pages().next().unwrap();
        let rendered = crate::markdown::render(
            &article.body,
            &crate::links::LinkIndex::default(),
            crate::markdown::RenderOptions::default(),
        )
        .unwrap();
        let html = Renderer::new(false)
            .unwrap()
            .article(&site, alpha, &[], loose, None, article, &rendered, &[])
            .unwrap();

        assert!(html.contains(">Loose</a>"));
        assert!(html.contains("aria-current=\"page\">Article x1</a>"));
        assert!(!html.contains("Acorn Docs"));
    }

    #[test]
    fn articles_without_headings_get_no_toc_and_no_scroll_spy() {
        let (_dir, site) = fixture_site();
        let alpha = site.products.iter().find(|p| p.slug == "alpha").unwrap();
        let acorn = alpha.anthologies().find(|d| d.slug == "acorn").unwrap();
        let wide = acorn.topics().next().unwrap();
        let article = wide.pages().next().unwrap();
        let rendered = crate::markdown::render(
            "just prose, no headings",
            &crate::links::LinkIndex::default(),
            crate::markdown::RenderOptions::default(),
        )
        .unwrap();
        let html = Renderer::new(false)
            .unwrap()
            .article(&site, alpha, &[acorn], wide, None, article, &rendered, &[])
            .unwrap();

        assert!(!html.contains("On this page"));
        assert!(!html.contains("Scroll-spy"));
    }

    #[test]
    fn pages_carry_the_search_modal_and_indexing_attributes() {
        let (_dir, site) = fixture_site();
        let renderer = Renderer::new(false).unwrap();
        let alpha = site.products.iter().find(|p| p.slug == "alpha").unwrap();

        // Every page gets the modal and the lazy search script.
        let front = renderer.index(&site).unwrap();
        assert!(front.contains("id=\"search-modal\""));
        assert!(front.contains("src=\"/assets/search.js\""));

        // Pages advertise their markdown mirror; the front page points at
        // the site map for AI consumers.
        assert!(front.contains("type=\"text/markdown\" href=\"/llms.txt\""));

        // The front page opts out of indexing; content pages opt in with
        // breadcrumb metadata for the result list.
        assert!(!front.contains("data-pagefind-body"));
        let product = renderer.product(&site, alpha).unwrap();
        assert!(product.contains("data-pagefind-body"));
        assert!(product.contains("type=\"text/markdown\" href=\"/alpha.md\""));

        let acorn = alpha.anthologies().find(|d| d.slug == "acorn").unwrap();
        let wide = acorn.topics().next().unwrap();
        let article = wide.pages().next().unwrap();
        let rendered = crate::markdown::render(
            &article.body,
            &crate::links::LinkIndex::default(),
            crate::markdown::RenderOptions::default(),
        )
        .unwrap();
        let html = renderer
            .article(&site, alpha, &[acorn], wide, None, article, &rendered, &[])
            .unwrap();
        assert!(html.contains("data-pagefind-body"));
        assert!(html.contains("data-pagefind-meta=\"crumbs:Alpha / Acorn Docs / Wide Topic\""));
        assert!(html.contains("<nav class=\"crumbs\" data-pagefind-ignore>"));

        let manual = alpha.books().find(|b| b.slug == "manual").unwrap();
        let (chapters, article) = manual
            .articles()
            .into_iter()
            .find(|(_, a)| a.slug == "tuning")
            .unwrap();
        let rendered = crate::markdown::render(
            &article.body,
            &crate::links::LinkIndex::default(),
            crate::markdown::RenderOptions {
                numbering: article.number.as_deref(),
                ..crate::markdown::RenderOptions::default()
            },
        )
        .unwrap();
        let html = renderer
            .book_article(
                &site,
                alpha,
                &[],
                manual,
                &chapters,
                article,
                &rendered,
                &[],
            )
            .unwrap();
        assert!(html.contains("data-pagefind-meta=\"crumbs:Alpha / AM / Setup / Advanced\""));
    }

    #[test]
    fn live_reload_script_is_dev_server_only() {
        let (_dir, site) = fixture_site();
        let with = Renderer::new(true).unwrap().index(&site).unwrap();
        let without = Renderer::new(false).unwrap().index(&site).unwrap();

        assert!(with.contains("EventSource(\"/~trail/reload\")"));
        assert!(!without.contains("EventSource"));
    }

    #[test]
    fn splits_the_sitename_for_the_wordmark() {
        assert_eq!(split_sitename("Peios Learn"), ("Peios", Some("Learn")));
        assert_eq!(split_sitename("Docs"), ("Docs", None));
    }
}
