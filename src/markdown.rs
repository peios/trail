use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

use anyhow::{Context, Result, bail, ensure};
use lol_html::{HtmlRewriter, RewriteStrSettings, Settings, element, rewrite_str};
use pulldown_cmark::{BlockQuoteKind, CodeBlockKind, Event, LinkType, Options, Tag, TagEnd, html};
use serde::{Deserialize, Serialize};

use crate::highlight;
use crate::images::{ImageInfo, ImageScope};
use crate::links::{LinkIndex, ResolveError};
use crate::refs::{InlineRefIndex, PhraseTarget, SectionTarget};

/// One "On this page" entry: an h2 or h3 with its generated anchor id.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct TocEntry {
    pub level: u8,
    pub id: String,
    pub title: String,
    /// The heading's section number ("2.1.3") when the page is numbered.
    pub number: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Rendered {
    pub html: String,
    pub toc: Vec<TocEntry>,
    /// Whether the article contains a mermaid diagram, so its page can pull
    /// in the (otherwise omitted) mermaid script.
    pub has_mermaid: bool,
    /// Dangling-link warnings — missing targets, missing `#fragment`
    /// anchors and relative paths when `allow_dangling` downgrades them.
    pub dangling: Vec<String>,
    /// Advisory link warnings that never make a build fail.
    pub warnings: Vec<String>,
    /// Fatal link problems: ambiguous references (always), and missing
    /// targets, missing anchors and relative paths when they aren't
    /// allowed. Collected rather than failing fast so one build reports
    /// every broken link at once; the caller decides when to abort.
    pub broken: Vec<String>,
}

/// How one article renders; see `render`.
#[derive(Debug, Default, Clone, Copy)]
pub struct RenderOptions<'a> {
    /// Downgrade missing ~link targets from render errors to collected
    /// warnings (ambiguous links always fail — the author has to pick).
    pub allow_dangling: bool,
    /// Warn about authored site-absolute links, which bypass ~ resolution.
    pub strict: bool,
    /// The page's section number inside a book ("2.1"): h2/h3 headings
    /// continue the dotted numbering as real text — searchable and
    /// copyable, unlike CSS counters — while ids and ToC titles stay
    /// clean of it.
    pub numbering: Option<&'a str>,
    /// Namespace for all HTML anchor ids, for pages that concatenate many
    /// articles (print pages) where slugs would otherwise collide.
    pub id_prefix: Option<&'a str>,
    /// Where relative `![](...)` destinations resolve: the site's image
    /// index plus this article's source directory. None (plain-markdown
    /// tests) leaves image events untouched.
    pub images: Option<ImageScope<'a>>,
    /// Inline-reference context: the phrase index plus this page's own
    /// identity. None (plain-markdown tests) skips phrase and `§`
    /// linking — RFC-2119 keywords highlight regardless.
    pub refs: Option<RefScope<'a>>,
    /// Set while rendering a `/print` bundle: links to pages inside the
    /// same bundle become in-document anchors, and everything else goes
    /// absolute. A printed page's links have to work on paper and away
    /// from the site.
    pub print: Option<PrintScope<'a>>,
    /// Prefix for every site-absolute URL this render emits — resolved
    /// `~` links, `§` section links, and the published URLs of images
    /// found in the tree. Empty for the site itself; "/<cbpath>" for the
    /// cache-busting copy, whose pages must stay inside the copy. URLs
    /// the author wrote out in full are never touched.
    pub base: &'a str,
}

/// Print-bundle link context; see `RenderOptions::print`.
#[derive(Debug, Clone, Copy)]
pub struct PrintScope<'a> {
    /// Page path → the bundle's anchor id for that page.
    pub anchors: &'a HashMap<String, String>,
    /// Precomputed once per bundle, not once for each article.
    pub cache_key: &'a str,
    /// The site's base URL, for pages outside the bundle. Without one
    /// they stay root-relative — still correct on the site itself.
    pub base: Option<&'a str>,
}

/// Inline-reference context for one page render; see `RenderOptions::refs`.
#[derive(Debug, Clone, Copy)]
pub struct RefScope<'a> {
    pub index: &'a InlineRefIndex,
    /// The page being rendered — a page never links a phrase to itself.
    pub page: &'a str,
    /// The path of the book this page belongs to, enabling bare `§`
    /// references to the book's own sections.
    pub book: Option<&'a str>,
    /// The book's short name ("PGSS", "Kernel TRM"), used to compose the
    /// full citation string a `[*name]` anchor displays.
    pub book_short: Option<&'a str>,
}

/// Render article markdown to HTML. Every heading gets a slugified,
/// deduplicated anchor id; h2/h3 headings are also collected into the ToC.
/// ```` ```mermaid ```` fences become `<pre class="mermaid">` blocks for
/// client-side mermaid to render. `~` link destinations are resolved
/// through the link index.
#[derive(Serialize, Deserialize)]
struct CachedRender {
    rendered: Rendered,
    images: Vec<std::path::PathBuf>,
    links: crate::links::Dependencies,
    refs: Option<crate::refs::SectionDependencies>,
}

pub fn render(markdown: &str, links: &LinkIndex, options: RenderOptions) -> Result<Rendered> {
    if !crate::cache::persistent() {
        return render_uncached(markdown, links, options);
    }
    let key = crate::cache::key_json(&(
        crate::memo::key(&[markdown.as_bytes()]),
        options.allow_dangling,
        options.strict,
        options.numbering,
        options.id_prefix,
        options.base,
        options.images.map(|s| (s.dir, s.index.cache_key())),
        options
            .refs
            .map(|s| (s.index.cache_key(), s.page, s.book, s.book_short)),
        options.print.map(|s| (s.cache_key, s.base)),
    ));
    if let Some(cached) = crate::cache::get::<CachedRender>("render", &key)
        && cached.links.valid(links)
        && cached
            .refs
            .as_ref()
            .zip(options.refs)
            .is_none_or(|(deps, scope)| deps.valid(scope.index))
    {
        cached.links.replay(links);
        if let Some((deps, scope)) = cached.refs.as_ref().zip(options.refs) {
            deps.replay(scope.index);
        }
        if let Some(scope) = options.images {
            scope.index.replay_usage(&cached.images);
        }
        return Ok(cached.rendered);
    }
    let start = options.images.map(|s| s.index.usage_start());
    let capture = links.capture();
    let ref_capture = options.refs.map(|scope| scope.index.capture());
    let result = render_uncached(markdown, links, options);
    let images = options
        .images
        .zip(start)
        .map(|(s, start)| s.index.usage_since(start))
        .unwrap_or_default();
    let cached = CachedRender {
        rendered: result?,
        images,
        links: capture.finish(),
        refs: ref_capture.map(|c| c.finish()),
    };
    crate::cache::put("render", &key, &cached);
    Ok(cached.rendered)
}

fn render_uncached(markdown: &str, links: &LinkIndex, options: RenderOptions) -> Result<Rendered> {
    let markdown = expand_tab_groups(markdown)?;
    // Before parsing: `[*name]` anchors become inline HTML, so the `*`
    // can never pair with an emphasis marker elsewhere in the block.
    let markdown =
        expand_citation_anchors(&markdown, options.refs.and_then(|scope| scope.book_short));
    // Equations first, so the typography pass never sees their source.
    let parsed = crate::parsed::parse(&markdown, parser_options());
    let events = crate::typography::keep_together(crate::math::mark_math(
        parsed
            .iter()
            .map(|(event, _)| crate::parsed::borrow_event(event))
            .collect(),
    ));

    let mut out = Vec::with_capacity(events.len());
    let mut toc = Vec::new();
    let mut used_ids = HashMap::new();
    let mut has_mermaid = false;
    let mut dangling = Vec::new();
    let mut warnings = Vec::new();
    let mut broken = Vec::new();
    // Same-page `#fragment` links, (as written, as emitted): checked
    // against the page's own ids once the whole body is rendered, since
    // a link may precede the heading it names.
    let mut local_fragments: Vec<(String, String)> = Vec::new();
    // Tracks open blockquotes: true = admonition we opened as an <aside>.
    let mut blockquotes = Vec::new();
    // Section-number counters, used only when `numbering` is set.
    let mut counter = SectionCounter::default();
    // Where prose enrichment (inline refs, `§`, RFC keywords) must not
    // reach: code blocks, headings (ids derive from their text), link
    // and image interiors.
    let (mut link_depth, mut image_depth) = (0u32, 0u32);
    // The open heading's anchor id, emitted as a permalink at its end.
    let mut heading_id: Option<String> = None;

    let mut i = 0;
    while i < events.len() {
        if let Some((figure_html, consumed)) = figure(&events[i..], &options) {
            out.push(Event::Html(figure_html.into()));
            i += consumed;
        } else if let Event::Start(Tag::Heading {
            level,
            id,
            classes,
            attrs,
        }) = &events[i]
        {
            let title = heading_text(&events[i + 1..]);
            let base = match id {
                Some(explicit) => explicit.to_string(),
                None => slugify(&title),
            };
            let unique = unique_id(&mut used_ids, base);
            heading_id = Some(unique.clone());
            let level_number = *level as u8;
            let number = counter.advance(options.numbering, level_number);
            if (2..=3).contains(&level_number) {
                toc.push(TocEntry {
                    level: level_number,
                    id: prefixed_id(options.id_prefix, &unique),
                    title,
                    number: number.clone(),
                });
            }
            out.push(Event::Start(Tag::Heading {
                level: *level,
                id: Some(unique.into()),
                classes: classes.clone(),
                attrs: attrs.clone(),
            }));
            if let Some(number) = number {
                out.push(Event::Html(
                    format!("<span class=\"heading-number\">{number}</span> ").into(),
                ));
            }
        } else if let Event::InlineMath(tex) | Event::DisplayMath(tex) = &events[i] {
            let display = matches!(events[i], Event::DisplayMath(_));
            match crate::math::to_mathml(tex, display) {
                Ok(mathml) if display => out.push(Event::Html(
                    format!("<div class=\"math-display\">{mathml}</div>\n").into(),
                )),
                Ok(mathml) => out.push(Event::InlineHtml(mathml.into())),
                Err(error) => {
                    // Not downgradable: the equation is the author's own text.
                    broken.push(format!("equation '{tex}' is not valid TeX: {error}"));
                    out.push(Event::Code(tex.clone()));
                }
            }
        } else if let Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(language))) = &events[i]
            && language.as_ref() == "mermaid"
        {
            let mut source = String::new();
            let mut j = i + 1;
            while j < events.len() {
                match &events[j] {
                    Event::End(TagEnd::CodeBlock) => break,
                    Event::Text(text) => source.push_str(text),
                    _ => {}
                }
                j += 1;
            }
            i = j;
            has_mermaid = true;
            out.push(Event::Html(
                format!("<pre class=\"mermaid\">{}</pre>\n", escape_text(&source)).into(),
            ));
        } else if let Event::Start(Tag::CodeBlock(kind)) = &events[i] {
            let language = match kind {
                CodeBlockKind::Fenced(info) => info
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .next()
                    .filter(|language| !language.is_empty()),
                CodeBlockKind::Indented => None,
            };
            let mut source = String::new();
            let mut j = i + 1;
            while j < events.len() {
                match &events[j] {
                    Event::End(TagEnd::CodeBlock) => break,
                    Event::Text(text) => source.push_str(text),
                    _ => {}
                }
                j += 1;
            }
            out.push(Event::Html(highlight::code_block(language, &source).into()));
            i = j;
        } else if let Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) = &events[i]
            && let Some(reference) = dest_url.strip_prefix('~')
        {
            link_depth += 1;
            let (target, fragment) = match reference.split_once('#') {
                Some((target, fragment)) => (target, Some(fragment)),
                None => (reference, None),
            };
            match links.resolve(target) {
                Ok(mut resolved) => {
                    // The page exists; does the anchor? Only pages with
                    // a body are known to the index — a product or
                    // anthology page is not checked.
                    if let Some(fragment) = fragment
                        && links.has_anchor(&resolved, fragment) == Some(false)
                    {
                        let message = format!(
                            "link '~{reference}' names anchor '#{fragment}', \
                             which page '{resolved}' does not have"
                        );
                        if options.allow_dangling {
                            dangling.push(message);
                        } else {
                            broken.push(message);
                        }
                    }
                    resolved = match options.print {
                        // Inside a print bundle: same-bundle pages become
                        // anchors (their headings keep the page's id
                        // prefix), others go absolute.
                        Some(scope) => match scope.anchors.get(&resolved) {
                            // Keep the canonical page path until the final
                            // HTML pass. That pass can then distinguish this
                            // cross-article link from an author's local href.
                            Some(_) => match fragment {
                                Some(fragment) => format!("{resolved}#{fragment}"),
                                None => resolved,
                            },
                            None => {
                                let site = scope.base.unwrap_or("");
                                let base = options.base;
                                match fragment {
                                    Some(fragment) => {
                                        format!("{site}{base}{resolved}#{fragment}")
                                    }
                                    None => format!("{site}{base}{resolved}"),
                                }
                            }
                        },
                        None => match fragment {
                            Some(fragment) => {
                                format!("{}{resolved}#{fragment}", options.base)
                            }
                            None => format!("{}{resolved}", options.base),
                        },
                    };
                    out.push(Event::Start(Tag::Link {
                        link_type: *link_type,
                        dest_url: resolved.into(),
                        title: title.clone(),
                        id: id.clone(),
                    }));
                }
                // Ambiguity is never downgradable — the author must pick
                // a candidate — but it is still collected, not thrown, so
                // one build surfaces every broken link together.
                Err(error @ ResolveError::Ambiguous(_)) => {
                    broken.push(error.to_string());
                    out.push(events[i].clone());
                }
                Err(error) if options.allow_dangling => {
                    dangling.push(error.to_string());
                    out.push(events[i].clone());
                }
                Err(error) => {
                    broken.push(error.to_string());
                    out.push(events[i].clone());
                }
            }
        } else if let Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) = &events[i]
            && let Some(fragment) = dest_url.strip_prefix('#')
        {
            link_depth += 1;
            // A same-page anchor. Inside a print bundle this page's ids
            // carry its prefix, so the link must too; either way the
            // fragment is checked against the page's own ids at the end.
            local_fragments.push((
                fragment.to_string(),
                prefixed_id(options.id_prefix, fragment),
            ));
            out.push(Event::Start(Tag::Link {
                link_type: *link_type,
                dest_url: dest_url.clone(),
                title: title.clone(),
                id: id.clone(),
            }));
        } else if let Event::Start(Tag::Link {
            link_type, dest_url, ..
        }) = &events[i]
            // An email autolink's destination is the bare address.
            && *link_type != LinkType::Email
            && is_relative(dest_url)
        {
            link_depth += 1;
            let message = format!(
                "link '{dest_url}' is a relative path, which no page can serve \
                 — use a ~ reference to the page you mean"
            );
            if options.allow_dangling {
                dangling.push(message);
            } else {
                broken.push(message);
            }
            out.push(events[i].clone());
        } else if let Event::Start(Tag::Image {
            dest_url, title, ..
        }) = &events[i]
        {
            match classify_image(dest_url, &options) {
                ImageFate::Local(info) => {
                    let (alt, end) = alt_text(&events[i + 1..]);
                    out.push(Event::Html(image_tag(&info, &alt, Some(title)).into()));
                    // Skip to the image's End tag; its alt events are
                    // flattened into the tag above.
                    i += 1 + end;
                }
                ImageFate::PassThrough => {
                    image_depth += 1;
                    out.push(events[i].clone());
                }
                ImageFate::Broken(message) => {
                    image_depth += 1;
                    broken.push(message);
                    out.push(events[i].clone());
                }
                ImageFate::Dangling(message) => {
                    image_depth += 1;
                    dangling.push(message);
                    out.push(events[i].clone());
                }
            }
        } else if let Event::Start(Tag::BlockQuote(kind)) = &events[i] {
            match kind {
                Some(kind) => {
                    let name = admonition_name(*kind);
                    blockquotes.push(true);
                    out.push(Event::Html(
                        format!(
                            "<aside class=\"admonition admonition-{}\">\n\
                             <p class=\"admonition-title\">{name}</p>\n",
                            name.to_ascii_lowercase()
                        )
                        .into(),
                    ));
                }
                None => {
                    // A plain quote opening with an alert-style marker is
                    // a typo'd admonition, not prose.
                    if let Some(name) = admonition_typo(&events[i + 1..]) {
                        bail!(
                            "unknown admonition type '[!{name}]' \
                             (expected NOTE, TIP, IMPORTANT, WARNING or CAUTION)"
                        );
                    }
                    blockquotes.push(false);
                    out.push(events[i].clone());
                }
            }
        } else if let Event::End(TagEnd::BlockQuote(_)) = &events[i] {
            if blockquotes.pop().unwrap_or(false) {
                out.push(Event::Html("</aside>\n".into()));
            } else {
                out.push(events[i].clone());
            }
        } else if let Event::Start(Tag::Table(_)) = &events[i] {
            // Tables scroll inside their own container: a wide table must
            // never force the whole page to scroll sideways.
            out.push(Event::Html("<div class=\"table-scroll\">\n".into()));
            out.push(events[i].clone());
        } else if let Event::End(TagEnd::Table) = &events[i] {
            out.push(events[i].clone());
            out.push(Event::Html("</div>\n".into()));
        } else if let Event::End(TagEnd::Heading(_)) = &events[i] {
            if let Some(id) = heading_id.take() {
                out.push(Event::Html(
                    format!(
                        " <a class=\"heading-anchor\" href=\"#{id}\" \
                         aria-label=\"Link to this section\">#</a>"
                    )
                    .into(),
                ));
            }
            out.push(events[i].clone());
        } else if let Event::Start(Tag::Link { dest_url, .. }) = &events[i] {
            link_depth += 1;
            if options.strict && dest_url.starts_with('/') && !dest_url.starts_with("//") {
                warnings.push(format!(
                    "link '{dest_url}' is site-absolute and bypasses page resolution \
                     — use a ~ reference for pages in this site"
                ));
            }
            out.push(events[i].clone());
        } else if let Event::End(TagEnd::Link) = &events[i] {
            link_depth = link_depth.saturating_sub(1);
            out.push(events[i].clone());
        } else if let Event::End(TagEnd::Image) = &events[i] {
            image_depth = image_depth.saturating_sub(1);
            out.push(events[i].clone());
        } else if let Event::Text(text) = &events[i] {
            let plain = link_depth == 0 && image_depth == 0 && heading_id.is_none();
            match plain
                .then(|| enrich_text(text, &options, &mut dangling, &mut broken))
                .flatten()
            {
                Some(html) => out.push(Event::Html(html.into())),
                None => out.push(events[i].clone()),
            }
        } else {
            out.push(events[i].clone());
        }
        i += 1;
    }

    let mut rendered = String::new();
    html::push_html(&mut rendered, out.into_iter());
    if options.id_prefix.is_some() || options.print.is_some() {
        rendered = namespace_html(&rendered, &options)?;
    }
    if !local_fragments.is_empty() {
        let ids = html_ids(&rendered)?;
        for (written, expected) in local_fragments {
            if ids.contains(&expected) {
                continue;
            }
            let message = format!("link '#{written}' names an anchor this page does not have");
            if options.allow_dangling {
                dangling.push(message);
            } else {
                broken.push(message);
            }
        }
    }
    Ok(Rendered {
        html: rendered,
        toc,
        has_mermaid,
        dangling,
        warnings,
        broken,
    })
}

/// Expand `:::tabs` groups into tab markup before parsing.
///
/// ```text
/// :::tabs
/// :::tab npm
/// (markdown)
/// :::tab pnpm
/// (markdown)
/// :::
/// ```
///
/// Each marker line becomes its own HTML block — followed by a blank
/// line, so CommonMark closes the block and every panel's body still
/// parses as ordinary markdown. Markers inside fenced code are left
/// alone, so documenting the syntax doesn't trigger it.
fn expand_tab_groups(markdown: &str) -> Result<String> {
    if !markdown.contains(":::") {
        return Ok(markdown.to_string());
    }
    let lines: Vec<&str> = markdown.lines().collect();
    let mut out = String::with_capacity(markdown.len() + 256);
    let mut fence: Option<&str> = None;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        // Track fenced code so markers inside samples stay literal.
        match fence {
            Some(open) => {
                if trimmed.starts_with(open) {
                    fence = None;
                }
            }
            None => {
                for marker in ["```", "~~~"] {
                    if trimmed.starts_with(marker) {
                        fence = Some(marker);
                        break;
                    }
                }
            }
        }
        if fence.is_some() || !trimmed.starts_with(":::") {
            out.push_str(line);
            out.push('\n');
            i += 1;
            continue;
        }
        if trimmed == ":::tabs" {
            i = expand_one_group(&lines, i, &mut out)?;
            continue;
        }
        if let Some(name) = trimmed.strip_prefix(":::tab") {
            bail!(
                "':::tab{name}' outside a ':::tabs' block \
                 (open the group with ':::tabs' first)"
            );
        }
        if trimmed == ":::" {
            bail!("':::' closes a tab group, but none is open");
        }
        out.push_str(line);
        out.push('\n');
        i += 1;
    }
    Ok(out)
}

/// Expand the group opening at `start`, returning the line index just
/// past its closing `:::`.
fn expand_one_group(lines: &[&str], start: usize, out: &mut String) -> Result<usize> {
    // Collect each tab's name and body lines.
    let mut tabs: Vec<(String, Vec<&str>)> = Vec::new();
    let mut fence: Option<&str> = None;
    let mut i = start + 1;
    let end = loop {
        let Some(line) = lines.get(i) else {
            bail!("unclosed ':::tabs' block (close it with ':::')");
        };
        let trimmed = line.trim();
        let in_code = match fence {
            Some(open) => {
                if trimmed.starts_with(open) {
                    fence = None;
                }
                true
            }
            None => {
                for marker in ["```", "~~~"] {
                    if trimmed.starts_with(marker) {
                        fence = Some(marker);
                        break;
                    }
                }
                fence.is_some()
            }
        };
        if !in_code {
            if trimmed == ":::" {
                break i;
            }
            if trimmed == ":::tabs" {
                bail!("':::tabs' groups cannot nest");
            }
            if let Some(name) = trimmed.strip_prefix(":::tab") {
                let name = name.trim();
                ensure!(
                    !name.is_empty(),
                    "':::tab' needs a label, e.g. ':::tab Linux'"
                );
                tabs.push((name.to_string(), Vec::new()));
                i += 1;
                continue;
            }
        }
        match tabs.last_mut() {
            Some((_, body)) => body.push(line),
            None => ensure!(
                trimmed.is_empty(),
                "content inside ':::tabs' before the first ':::tab' label"
            ),
        }
        i += 1;
    };
    ensure!(
        !tabs.is_empty(),
        "':::tabs' block has no ':::tab' labels in it"
    );

    out.push_str(
        "\n<div class=\"tabs\" data-tabs>\n<div class=\"tab-buttons\" role=\"tablist\">\n",
    );
    for (index, (name, _)) in tabs.iter().enumerate() {
        let selected = index == 0;
        let _ = writeln!(
            out,
            "<button class=\"tab-button\" type=\"button\" role=\"tab\" \
             data-tab=\"{index}\" aria-selected=\"{selected}\">{}</button>",
            escape_text(name)
        );
    }
    out.push_str("</div>\n\n");
    for (index, (_, body)) in tabs.iter().enumerate() {
        let hidden = if index == 0 { "" } else { " hidden" };
        let _ = write!(
            out,
            "<div class=\"tab-panel\" role=\"tabpanel\" data-tab-panel=\"{index}\"{hidden}>\n\n"
        );
        for line in body {
            out.push_str(line);
            out.push('\n');
        }
        out.push_str("\n</div>\n\n");
    }
    out.push_str("</div>\n\n");
    Ok(end + 1)
}

/// h2/h3 section-number counters ("2.1" → "2.1.1", "2.1.1.1"), shared by
/// every pass that numbers headings so they can never drift apart.
#[derive(Debug, Default)]
struct SectionCounter {
    h2: u32,
    h3: u32,
}

impl SectionCounter {
    fn advance(&mut self, numbering: Option<&str>, level: u8) -> Option<String> {
        match (numbering, level) {
            (Some(prefix), 2) => {
                self.h2 += 1;
                self.h3 = 0;
                Some(format!("{prefix}.{}", self.h2))
            }
            (Some(prefix), 3) => {
                self.h3 += 1;
                Some(format!("{prefix}.{}.{}", self.h2, self.h3))
            }
            _ => None,
        }
    }
}

/// The headings of an article without rendering it: the same ids and
/// section numbers `render` would assign, for building section indexes
/// ahead of the render pass. h2/h3 only — deeper headings consume ids
/// but never numbers, exactly as in `render`.
pub fn heading_outline(markdown: &str, numbering: Option<&str>) -> Vec<TocEntry> {
    let key = crate::memo::key(&[
        markdown.as_bytes(),
        &[u8::from(numbering.is_some())],
        numbering.unwrap_or("").as_bytes(),
    ]);
    crate::cache::compute("headings", &key, || {
        heading_outline_uncached(markdown, numbering)
    })
}

fn heading_outline_uncached(markdown: &str, numbering: Option<&str>) -> Vec<TocEntry> {
    let parsed = crate::parsed::parse(markdown, parser_options());
    let events: Vec<Event> = parsed
        .iter()
        .map(|(event, _)| crate::parsed::borrow_event(event))
        .collect();
    let mut outline = Vec::new();
    let mut used_ids = HashMap::new();
    let mut counter = SectionCounter::default();
    for (i, event) in events.iter().enumerate() {
        let Event::Start(Tag::Heading { level, id, .. }) = event else {
            continue;
        };
        let title = heading_text(&events[i + 1..]);
        let base = match id {
            Some(explicit) => explicit.to_string(),
            None => slugify(&title),
        };
        let unique = unique_id(&mut used_ids, base);
        let level_number = *level as u8;
        let number = counter.advance(numbering, level_number);
        if (2..=3).contains(&level_number) {
            outline.push(TocEntry {
                level: level_number,
                id: unique,
                title,
                number,
            });
        }
    }
    outline
}

/// Enrich one plain-prose text segment: inline-reference phrases become
/// links (a book phrase optionally reaching a `§` section), bare `§`
/// references resolve within the surrounding book, and RFC-2119
/// requirement keywords get highlighted. None means untouched — the
/// caller keeps the original text event.
fn enrich_text(
    text: &str,
    options: &RenderOptions,
    dangling: &mut Vec<String>,
    broken: &mut Vec<String>,
) -> Option<String> {
    let mut spans: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    if let Some(scope) = &options.refs {
        phrase_spans(
            text,
            scope,
            options.allow_dangling,
            options.base,
            &mut spans,
            dangling,
            broken,
        );
        if let Some(book) = scope.book {
            bare_section_spans(text, scope, book, options.base, &mut spans);
        }
    }
    rfc_keyword_spans(text, &mut spans);
    if spans.is_empty() {
        return None;
    }
    spans.sort_by_key(|(range, _)| range.start);
    let mut out = String::with_capacity(text.len() + spans.len() * 32);
    let mut at = 0;
    for (range, html) in spans {
        out.push_str(&escape_text(&text[at..range.start]));
        out.push_str(&html);
        at = range.end;
    }
    out.push_str(&escape_text(&text[at..]));
    Some(out)
}

/// Inline-reference phrase matches: whole words only, longest phrase
/// first, never linking a page to itself.
fn phrase_spans(
    text: &str,
    scope: &RefScope,
    allow_dangling: bool,
    base: &str,
    spans: &mut Vec<(std::ops::Range<usize>, String)>,
    dangling: &mut Vec<String>,
    broken: &mut Vec<String>,
) {
    let Some(matcher) = scope.index.matcher() else {
        return;
    };
    for hit in matcher.find_iter(text) {
        let (start, mut end) = (hit.start(), hit.end());
        if !boundary_before(text, start) || !boundary_after(text, end) {
            continue;
        }
        let phrase = scope.index.phrase(hit.pattern().as_usize());
        let href = match &phrase.target {
            PhraseTarget::Page(path) => {
                if path == scope.page {
                    continue;
                }
                format!("{base}{path}")
            }
            PhraseTarget::Book(book) => match section_suffix(&text[end..]) {
                Some((consumed, label)) => match scope.index.section(book, label) {
                    Some(target) => {
                        end += consumed;
                        match section_href(target, scope.page, base) {
                            Some(href) => href,
                            // The section is this very page: leave it plain.
                            None => continue,
                        }
                    }
                    None => {
                        let message = format!(
                            "'{} §{label}' does not match any section of '{book}'",
                            &text[start..end]
                        );
                        if allow_dangling {
                            dangling.push(message);
                        } else {
                            broken.push(message);
                        }
                        // Still link the phrase itself to the book.
                        if book == scope.page {
                            continue;
                        }
                        format!("{base}{book}")
                    }
                },
                None => {
                    if book == scope.page {
                        continue;
                    }
                    format!("{base}{book}")
                }
            },
        };
        spans.push((start..end, ref_link(&href, &text[start..end])));
    }
}

/// Bare `§<number>` references, resolved against the surrounding book.
/// A number that resolves becomes a link; one that doesn't stays plain
/// with no complaint — prose regularly cites *external* documents'
/// section numbers (prior-art discussions), so only the phrase-qualified
/// form ("PGSS §9.9"), which names its book, insists on resolving.
fn bare_section_spans(
    text: &str,
    scope: &RefScope,
    book: &str,
    base: &str,
    spans: &mut Vec<(std::ops::Range<usize>, String)>,
) {
    for (at, _) in text.match_indices('§') {
        if covered(spans, at) || !boundary_before(text, at) {
            continue;
        }
        let rest = &text[at + '§'.len_utf8()..];
        let Some(label) = section_label(rest) else {
            // A lone § sign is just prose.
            continue;
        };
        let end = at + '§'.len_utf8() + label.len();
        if let Some(target) = scope.index.section(book, label)
            && let Some(href) = section_href(target, scope.page, base)
        {
            spans.push((at..end, ref_link(&href, &text[at..end])));
        }
    }
}

/// RFC-2119 requirement keywords, longest first so "MUST NOT" wins
/// before "MUST".
const RFC_KEYWORDS: &[&str] = &[
    "MUST NOT",
    "SHALL NOT",
    "SHOULD NOT",
    "NOT RECOMMENDED",
    "RECOMMENDED",
    "REQUIRED",
    "OPTIONAL",
    "SHOULD",
    "SHALL",
    "MUST",
    "MAY",
];

fn rfc_keyword_spans(text: &str, spans: &mut Vec<(std::ops::Range<usize>, String)>) {
    let mut at = 0;
    while at < text.len() {
        if !text.is_char_boundary(at) || !boundary_before(text, at) || covered(spans, at) {
            at += 1;
            continue;
        }
        let Some(keyword) = RFC_KEYWORDS.iter().find(|keyword| {
            text[at..].starts_with(**keyword)
                && boundary_after(text, at + keyword.len())
                && !covered(spans, at + keyword.len() - 1)
        }) else {
            at += 1;
            continue;
        };
        spans.push((
            at..at + keyword.len(),
            format!("<span class=\"rfc-keyword\">{keyword}</span>"),
        ));
        at += keyword.len();
    }
}

/// The href for a section target, from the referencing page: a same-page
/// section links by fragment alone; a same-page reference with no
/// fragment has nowhere to go (None — the text stays plain).
fn section_href(target: &SectionTarget, page: &str, base: &str) -> Option<String> {
    match (&target.fragment, target.path == page) {
        (Some(fragment), true) => Some(format!("#{fragment}")),
        (None, true) => None,
        (Some(fragment), false) => Some(format!("{base}{}#{fragment}", target.path)),
        (None, false) => Some(format!("{base}{}", target.path)),
    }
}

/// Parse a ` §<number>` suffix directly after a book phrase: one space
/// (or no-break space), the sign, and a section label. Returns the byte
/// length consumed and the label.
fn section_suffix(rest: &str) -> Option<(usize, &str)> {
    let after_space = rest
        .strip_prefix(' ')
        .or_else(|| rest.strip_prefix('\u{a0}'))?;
    let after_sign = after_space.strip_prefix('§')?;
    let label = section_label(after_sign)?;
    Some((rest.len() - after_sign.len() + label.len(), label))
}

/// A section label: digits, dots and appendix letters ("2.1", "A.3",
/// "2.A"), a trailing sentence dot excluded, ending at a word boundary.
fn section_label(text: &str) -> Option<&str> {
    let end = text
        .find(|c: char| !(c.is_ascii_digit() || c.is_ascii_uppercase() || c == '.'))
        .unwrap_or(text.len());
    let label = text[..end].trim_end_matches('.');
    if label.is_empty() || !boundary_after(text, end) {
        return None;
    }
    Some(label)
}

fn boundary_before(text: &str, at: usize) -> bool {
    text[..at]
        .chars()
        .next_back()
        .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
}

fn boundary_after(text: &str, at: usize) -> bool {
    text[at..]
        .chars()
        .next()
        .is_none_or(|c| !(c.is_alphanumeric() || c == '_'))
}

fn covered(spans: &[(std::ops::Range<usize>, String)], at: usize) -> bool {
    spans.iter().any(|(range, _)| range.contains(&at))
}

fn ref_link(href: &str, text: &str) -> String {
    format!(
        "<a class=\"inline-ref\" href=\"{}\">{}</a>",
        escape_attribute(href),
        escape_text(text)
    )
}

/// Detect `[!NAME]` at the start of an unrecognised blockquote: pulldown
/// only parses the five GFM alert kinds, so anything still carrying the
/// marker here is a typo the author should hear about.
fn admonition_typo(events: &[Event]) -> Option<String> {
    // The pulldown parser splits brackets into their own text events
    // (they might have opened a link), so gather the quote's first line
    // before looking for the marker.
    let mut first_line = String::new();
    for event in events {
        match event {
            Event::Start(Tag::Paragraph) => continue,
            Event::Text(text) => first_line.push_str(text),
            _ => break,
        }
    }
    let rest = first_line.strip_prefix("[!")?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    (!name.is_empty() && rest[name.len()..].starts_with(']')).then_some(name)
}

/// How one image destination renders; see `classify_image`.
enum ImageFate {
    /// A tree image: emit our own tag with its published URL (and size).
    Local(ImageInfo),
    /// External, explicitly absolute, or no scope to resolve against —
    /// leave pulldown's own rendering alone.
    PassThrough,
    Broken(String),
    Dangling(String),
}

/// Classify an image destination. Relative paths resolve against the
/// article's source directory through the image index; `~` is a page
/// reference and never an image. Pure lookup — the caller records any
/// problem, so the figure lookahead and the inline pass can both
/// classify without double-reporting.
fn classify_image(dest: &str, options: &RenderOptions) -> ImageFate {
    if dest.starts_with('~') {
        return ImageFate::Broken(format!(
            "image '{dest}': ~references name pages, not files — \
             use a path relative to the article"
        ));
    }
    if dest.starts_with("http://")
        || dest.starts_with("https://")
        || dest.starts_with("data:")
        || dest.starts_with('/')
    {
        return ImageFate::PassThrough;
    }
    let Some(scope) = options.images else {
        return ImageFate::PassThrough;
    };
    match scope.index.resolve(scope.dir, dest) {
        Some(info) => ImageFate::Local(ImageInfo {
            url: format!("{}{}", options.base, info.url),
            ..info.clone()
        }),
        None => {
            let message =
                format!("image '{dest}' not found (resolved relative to the article's folder)");
            if options.allow_dangling {
                ImageFate::Dangling(message)
            } else {
                ImageFate::Broken(message)
            }
        }
    }
}

/// A paragraph holding nothing but one captioned image — `![alt](src
/// "Caption")` on its own line — becomes a `<figure>` with the caption
/// as its `<figcaption>` (an inline `<img>` inside a `<p>` couldn't
/// legally hold one). Returns the figure and the number of events it
/// replaces, or None to let the paragraph render normally: uncaptioned,
/// unresolvable (the inline pass reports it once), or the image shares
/// its paragraph with other text.
fn figure(events: &[Event], options: &RenderOptions) -> Option<(String, usize)> {
    let Some(Event::Start(Tag::Paragraph)) = events.first() else {
        return None;
    };
    let Some(Event::Start(Tag::Image {
        dest_url, title, ..
    })) = events.get(1)
    else {
        return None;
    };
    if title.is_empty() {
        return None;
    }
    let (alt, end) = alt_text(&events[2..]);
    let end_image = 2 + end;
    let Some(Event::End(TagEnd::Paragraph)) = events.get(end_image + 1) else {
        return None;
    };
    let info = match classify_image(dest_url, options) {
        ImageFate::Local(info) => info,
        // External images can be figures too; no dimensions to give.
        ImageFate::PassThrough => ImageInfo {
            url: dest_url.to_string(),
            width: None,
            height: None,
        },
        ImageFate::Broken(_) | ImageFate::Dangling(_) => return None,
    };
    let html = format!(
        "<figure>{}<figcaption>{}</figcaption></figure>\n",
        image_tag(&info, &alt, None),
        escape_text(title),
    );
    Some((html, end_image + 1))
}

/// Flatten an image's alt-text events (everything before its End tag)
/// to plain text, the way an HTML alt attribute flattens markup.
/// Returns the text and the End tag's index within the slice.
fn alt_text(events: &[Event]) -> (String, usize) {
    let mut alt = String::new();
    let mut i = 0;
    while i < events.len() {
        match &events[i] {
            Event::End(TagEnd::Image) => break,
            Event::Text(text) | Event::Code(text) => alt.push_str(text),
            Event::SoftBreak | Event::HardBreak => alt.push(' '),
            _ => {}
        }
        i += 1;
    }
    (alt, i)
}

fn image_tag(info: &ImageInfo, alt: &str, title: Option<&str>) -> String {
    let mut tag = format!(
        "<img src=\"{}\" alt=\"{}\"",
        escape_attribute(&info.url),
        escape_attribute(alt)
    );
    if let (Some(width), Some(height)) = (info.width, info.height) {
        let _ = write!(tag, " width=\"{width}\" height=\"{height}\"");
    }
    if let Some(title) = title.filter(|title| !title.is_empty()) {
        let _ = write!(tag, " title=\"{}\"", escape_attribute(title));
    }
    tag.push('>');
    tag
}

/// ENABLE_GFM parses `> [!NOTE]`-style blockquote alerts (admonitions).
/// ENABLE_MATH stays off: a pair of `$` in ordinary prose ("$PATH and
/// $HOME") would become an equation. `math::mark_math` finds equations in
/// forms prose never takes.
pub(crate) fn parser_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_GFM
}

/// Rewrite article markdown for the AI-facing mirrors, preserving the
/// source formatting: inline `~` link destinations become the target
/// page's own mirror URL ("/alpha/acorn/wide/a2.md"), relative image
/// destinations become the image's published URL (mirrors and bundles
/// serve the body away from its own folder), and with `numbering` set
/// the h2/h3 headings gain their section numbers as text, exactly as
/// `render` numbers them. Anything that doesn't resolve is left
/// untouched — the HTML render has already reported or rejected it, so
/// this pass never fails.
pub fn rewrite_source(
    markdown: &str,
    links: &LinkIndex,
    numbering: Option<&str>,
    images: Option<ImageScope>,
    base: &str,
) -> String {
    if !crate::cache::persistent() {
        return rewrite_source_uncached(markdown, links, numbering, images, base);
    }
    let key = crate::cache::key_json(&(
        crate::memo::key(&[markdown.as_bytes()]),
        numbering,
        base,
        images.map(|s| (s.dir, s.index.cache_key())),
    ));
    if let Some((text, used, dependencies)) =
        crate::cache::get::<(String, Vec<std::path::PathBuf>, crate::links::Dependencies)>(
            "source", &key,
        )
        && dependencies.valid(links)
    {
        dependencies.replay(links);
        if let Some(scope) = images {
            scope.index.replay_usage(&used);
        }
        return text;
    }
    let start = images.map(|s| s.index.usage_start());
    let capture = links.capture();
    let text = rewrite_source_uncached(markdown, links, numbering, images, base);
    let used = images
        .zip(start)
        .map(|(s, start)| s.index.usage_since(start))
        .unwrap_or_default();
    crate::cache::put("source", &key, &(&text, used, capture.finish()));
    text
}

fn rewrite_source_uncached(
    markdown: &str,
    links: &LinkIndex,
    numbering: Option<&str>,
    images: Option<ImageScope>,
    base: &str,
) -> String {
    let mut edits: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    let mut counter = SectionCounter::default();
    for (event, range) in crate::parsed::parse(markdown, parser_options())
        .iter()
        .map(|(event, range)| (crate::parsed::borrow_event(event), range.clone()))
    {
        match event {
            Event::Start(Tag::Link { dest_url, .. }) if dest_url.starts_with('~') => {
                let reference = &dest_url[1..];
                let (target, fragment) = match reference.split_once('#') {
                    Some((target, fragment)) => (target, Some(fragment)),
                    None => (reference, None),
                };
                let Ok(resolved) = links.resolve(target) else {
                    continue;
                };
                let resolved = links.source_url(&resolved);
                let new_dest = match fragment {
                    Some(fragment) => format!("{base}{resolved}#{fragment}"),
                    None => format!("{base}{resolved}"),
                };
                if let Some(dest_range) = inline_dest_range(markdown, &range, &dest_url) {
                    edits.push((dest_range, new_dest));
                }
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                if dest_url.starts_with('~')
                    || dest_url.starts_with("http://")
                    || dest_url.starts_with("https://")
                    || dest_url.starts_with("data:")
                    || dest_url.starts_with('/')
                {
                    continue;
                }
                let Some(scope) = images else { continue };
                let Some(info) = scope.index.resolve(scope.dir, &dest_url) else {
                    continue;
                };
                if let Some(dest_range) = inline_dest_range(markdown, &range, &dest_url) {
                    edits.push((dest_range, format!("{base}{}", info.url)));
                }
            }
            Event::Start(Tag::Heading { level, .. }) if numbering.is_some() => {
                let Some(number) = counter.advance(numbering, level as u8) else {
                    continue;
                };
                // ATX headings only: insert after the "## " prefix.
                let source = &markdown[range.clone()];
                let hashes = source.bytes().take_while(|b| *b == b'#').count();
                if hashes == 0 || source.as_bytes().get(hashes) != Some(&b' ') {
                    continue;
                }
                let at = range.start + hashes + 1;
                edits.push((at..at, format!("{number} ")));
            }
            _ => {}
        }
    }
    // Attachment reference definitions also need file URLs in exported text.
    // Keep the ordinary parser fast path for pages without attachment links.
    if markdown.contains("~attachments/") {
        let parser = pulldown_cmark::Parser::new_ext(markdown, parser_options());
        for (_, definition) in parser.reference_definitions().iter() {
            let Some(reference) = definition.dest.strip_prefix('~') else {
                continue;
            };
            if !reference.starts_with("attachments/") {
                continue;
            }
            let (target, fragment) = reference
                .split_once('#')
                .map_or((reference, None), |(t, f)| (t, Some(f)));
            let Ok(resolved) = links.resolve(target) else {
                continue;
            };
            let source = &markdown[definition.span.clone()];
            let Some(label_end) = source.find("]:") else {
                continue;
            };
            let start = label_end + 2;
            let Some(offset) = source[start..].find(definition.dest.as_ref()) else {
                continue;
            };
            let at = definition.span.start + start + offset;
            let destination = match fragment {
                Some(fragment) => format!("{base}{resolved}#{fragment}"),
                None => format!("{base}{resolved}"),
            };
            edits.push((at..at + definition.dest.len(), destination));
        }
    }
    let mut out = markdown.to_string();
    edits.sort_by_key(|edit| std::cmp::Reverse(edit.0.start));
    for (range, replacement) in edits {
        out.replace_range(range, &replacement);
    }
    out
}

/// The byte range of an inline link/image destination inside its event
/// range — "[text](dest)", "![alt](dest \"title\")" — for source
/// rewriting. Reference-style links keep their destination elsewhere
/// and get None; a title after the destination is tolerated and kept.
fn inline_dest_range(
    markdown: &str,
    range: &std::ops::Range<usize>,
    dest_url: &str,
) -> Option<std::ops::Range<usize>> {
    let source = &markdown[range.clone()];
    let open = source.rfind("](")?;
    if !source.ends_with(')') {
        return None;
    }
    let inner = (range.start + open + 2)..(range.end - 1);
    let written = &markdown[inner.clone()];
    if let Some(rest) = written
        .strip_prefix('<')
        .and_then(|text| text.strip_prefix(dest_url))
        && let Some(rest) = rest.strip_prefix('>')
        && (rest.is_empty() || rest.starts_with([' ', '\t']))
    {
        return Some(inner.start + 1..inner.start + 1 + dest_url.len());
    }
    if written == dest_url {
        return Some(inner);
    }
    let rest = written.strip_prefix(dest_url)?;
    rest.starts_with([' ', '\t'])
        .then(|| inner.start..inner.start + dest_url.len())
}

fn admonition_name(kind: BlockQuoteKind) -> &'static str {
    match kind {
        BlockQuoteKind::Note => "Note",
        BlockQuoteKind::Tip => "Tip",
        BlockQuoteKind::Important => "Important",
        BlockQuoteKind::Warning => "Warning",
        BlockQuoteKind::Caution => "Caution",
    }
}

/// Escape diagram source for embedding as element text; the browser decodes
/// the entities back before mermaid reads the element's text content.
pub(crate) fn escape_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape text for an HTML attribute value (double-quoted).
fn escape_attribute(text: &str) -> String {
    escape_text(text).replace('"', "&quot;")
}

/// The text of the heading whose start event precedes `events`, as its
/// id and ToC title see it: text and code runs, nothing else.
fn heading_text(events: &[Event]) -> String {
    let mut title = String::new();
    for event in events {
        match event {
            Event::End(TagEnd::Heading(_)) => break,
            Event::Text(text) | Event::Code(text) => title.push_str(text),
            _ => {}
        }
    }
    title
}

/// Apply one namespace to the complete rendered article, including raw
/// HTML and citation markers. Parsing the whole body preserves HTML context
/// across markdown events (multiline tags, comments and script/raw-text
/// contents); matching attributes avoids rewriting code or quoted examples.
fn namespace_html(html: &str, options: &RenderOptions) -> Result<String> {
    rewrite_str(
        html,
        RewriteStrSettings {
            element_content_handlers: vec![element!("[id], [href]", |element| {
                if let Some(id) = element.get_attribute("id") {
                    let id = html_escape::decode_html_entities(&id);
                    if !id.is_empty() && options.id_prefix.is_some() {
                        element.set_attribute(
                            "id",
                            &escape_attribute(&prefixed_id(options.id_prefix, &id)),
                        )?;
                    }
                }
                if let Some(href) = element.get_attribute("href") {
                    let href = html_escape::decode_html_entities(&href);
                    let destination = if let Some(fragment) = href.strip_prefix('#') {
                        // A bare '#' still points at the document top.
                        (!fragment.is_empty() && options.id_prefix.is_some())
                            .then(|| format!("#{}", prefixed_id(options.id_prefix, fragment)))
                    } else {
                        // Only exact canonical page paths are scoped. Full
                        // URLs, query strings and out-of-bundle paths retain
                        // their authored meaning. This also handles raw HTML
                        // links to another article in the same bundle.
                        let (path, fragment) = href
                            .split_once('#')
                            .map_or((href.as_ref(), None), |(path, fragment)| {
                                (path, Some(fragment))
                            });
                        options
                            .print
                            .and_then(|scope| scope.anchors.get(path))
                            .map(|anchor| match fragment {
                                Some(fragment) if !fragment.is_empty() => {
                                    format!("#{anchor}--{fragment}")
                                }
                                _ => format!("#{anchor}"),
                            })
                    };
                    if let Some(destination) = destination {
                        element.set_attribute("href", &escape_attribute(&destination))?;
                    }
                }
                Ok(())
            })],
            ..RewriteStrSettings::default()
        },
    )
    .context("could not namespace print article HTML")
}

fn prefixed_id(prefix: Option<&str>, id: &str) -> String {
    match prefix {
        Some(prefix) => format!("{prefix}--{id}"),
        None => id.to_string(),
    }
}

/// Actual element IDs, including quoted/unquoted attributes and entities.
/// Unlike a substring search, comments and raw-text examples are not anchors.
fn html_ids(html: &str) -> Result<HashSet<String>> {
    let mut ids = HashSet::new();
    let mut rewriter = HtmlRewriter::new(
        Settings {
            element_content_handlers: vec![element!("[id]", |element| {
                if let Some(id) = element.get_attribute("id") {
                    let id = html_escape::decode_html_entities(&id);
                    if !id.is_empty() {
                        ids.insert(id.into_owned());
                    }
                }
                Ok(())
            })],
            ..Settings::default()
        },
        |_: &[u8]| {},
    );
    rewriter
        .write(html.as_bytes())
        .context("could not collect article HTML anchors")?;
    rewriter
        .end()
        .context("could not collect article HTML anchors")?;
    Ok(ids)
}

/// Every anchor id an article body defines — heading ids exactly as
/// `render` assigns them (an explicit `{#id}` or the slugified text,
/// deduplicated in order), `[*name]` citation anchors, and ids written
/// in raw HTML. The link index collects these so a `#fragment` on a
/// reference can be checked against the page it names.
pub fn anchor_ids(markdown: &str) -> HashSet<String> {
    let key = crate::memo::key(&[markdown.as_bytes()]);
    crate::cache::compute("anchors", &key, || anchor_ids_uncached(markdown))
}

fn anchor_ids_uncached(markdown: &str) -> HashSet<String> {
    let expanded = expand_tab_groups(markdown).unwrap_or_else(|_| markdown.to_string());
    let expanded = expand_citation_anchors(&expanded, None);
    let parsed = crate::parsed::parse(&expanded, parser_options());
    let events: Vec<Event> = parsed
        .iter()
        .map(|(event, _)| crate::parsed::borrow_event(event))
        .collect();
    let mut ids = HashSet::new();
    let mut used_ids = HashMap::new();
    for (i, event) in events.iter().enumerate() {
        if let Event::Start(Tag::Heading { id, .. }) = event {
            let base = match id {
                Some(explicit) => explicit.to_string(),
                None => slugify(&heading_text(&events[i + 1..])),
            };
            ids.insert(unique_id(&mut used_ids, base));
        }
    }
    // Parse the complete HTML rather than isolated raw events, which can
    // split a tag or a comment over several lines. If HTML is malformed in
    // a way the rewriter cannot handle, do not index unverifiable raw ids;
    // rendering with local links or a print scope reports the parse error.
    let mut html = String::new();
    html::push_html(&mut html, events.into_iter());
    ids.extend(html_ids(&html).unwrap_or_default());
    ids
}

/// The anchors an article body names itself: ids in raw HTML, and
/// citation anchors. Every id except the slugs Trail derives from heading
/// text, which change whenever a heading is reworded, so these are the
/// targets other content can rely on.
pub fn explicit_anchor_ids(markdown: &str) -> HashSet<String> {
    let key = crate::memo::key(&[markdown.as_bytes()]);
    crate::cache::compute("explicit-anchors", &key, || {
        explicit_anchor_ids_uncached(markdown)
    })
}

fn explicit_anchor_ids_uncached(markdown: &str) -> HashSet<String> {
    let expanded = expand_tab_groups(markdown).unwrap_or_else(|_| markdown.to_string());
    let expanded = expand_citation_anchors(&expanded, None);
    let mut html = String::new();
    html::push_html(
        &mut html,
        crate::parsed::parse(&expanded, parser_options())
            .iter()
            .map(|(event, _)| crate::parsed::borrow_event(event)),
    );
    html_ids(&html).unwrap_or_default()
}

/// A link destination in none of the forms a page can serve: not a `~`
/// reference, not site-absolute, not a same-page fragment, and not a URL
/// with a scheme. A page's URL comes from its slugs, not its file, so a
/// path relative to the source file points at nothing.
fn is_relative(dest: &str) -> bool {
    if dest.starts_with(['~', '/', '#']) {
        return false;
    }
    let Some(colon) = dest.find(':') else {
        return true;
    };
    let scheme = &dest[..colon];
    let well_formed = scheme.chars().enumerate().all(|(i, c)| {
        c.is_ascii_alphabetic() || (i > 0 && (c.is_ascii_digit() || matches!(c, '+' | '-' | '.')))
    });
    scheme.is_empty() || !well_formed
}

fn slugify(text: &str) -> String {
    let mut out = String::new();
    let mut previous_was_dash = true;
    for c in text.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
            previous_was_dash = false;
        } else if !previous_was_dash {
            out.push('-');
            previous_was_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "section".into()
    } else {
        out
    }
}

fn unique_id(used: &mut HashMap<String, usize>, base: String) -> String {
    let count = used.entry(base.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base
    } else {
        format!("{base}-{count}")
    }
}

// ---------------------------------------------------------------------
// Named Citations: `[*name]` anchors on an individual statement.
//
// An anchor is a *locator*, not a delimiter: it names a place in the
// prose so a test, a code comment or a coverage report can point at it.
// It carries no notion of where the statement it marks begins or ends —
// see the Named Citations documentation for why extent is deliberately
// not modelled.
//
// Anchors are expanded before the markdown is parsed. The `*` in
// `[*name]` is left-flanking and can therefore be closed by an emphasis
// `*` elsewhere in the same paragraph, which would swallow the anchor
// into an <em> and lose it. Rewriting to inline HTML up front means the
// parser never sees the asterisk. Fenced blocks and inline code are
// skipped so documentation *about* the syntax stays literal.
// ---------------------------------------------------------------------

/// One `[*name]` anchor found in an article's source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationAnchor {
    /// The declared name, e.g. `copy-up.preserves-ownership`.
    pub name: String,
    /// The source block the anchor sits in, anchors removed. This is
    /// *block context*, not the statement: an anchor does not delimit
    /// one, and a block holding several anchors gives each the same
    /// context. Split the block if that is too coarse.
    pub context: String,
    /// True when the anchor sits on a heading, naming the whole section
    /// rather than one statement inside it. A consumer wants the
    /// difference: an uncovered section is a different-sized gap from
    /// an uncovered sentence.
    pub section: bool,
}

/// A citation name: lowercase alphanumerics in dot- or hyphen-separated
/// segments, starting and ending alphanumeric. Enforced rather than
/// merely conventional — a corpus this size otherwise drifts into
/// three naming styles, and the name is what every citation greps for.
pub(crate) fn citation_name(text: &str) -> Option<&str> {
    let end = text
        .find(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-'))
        .unwrap_or(text.len());
    let name = &text[..end];
    if name.is_empty() {
        return None;
    }
    let first = name.as_bytes()[0];
    let last = name.as_bytes()[name.len() - 1];
    if !first.is_ascii_alphanumeric() || !last.is_ascii_alphanumeric() {
        return None;
    }
    if name.contains("..") || name.contains("--") || name.contains(".-") || name.contains("-.") {
        return None;
    }
    Some(name)
}

/// Byte ranges of every `[*name]` anchor in `markdown`, with its name.
/// Fenced code blocks and inline code spans are skipped.
pub(crate) fn citation_spans(markdown: &str) -> Vec<(std::ops::Range<usize>, String)> {
    let mut found = Vec::new();
    if !markdown.contains("[*") {
        return found;
    }
    let mut fence: Option<&str> = None;
    let mut offset = 0usize;
    for line in markdown.split_inclusive('\n') {
        let trimmed = line.trim();
        match fence {
            Some(open) => {
                if trimmed.starts_with(open) {
                    fence = None;
                }
                offset += line.len();
                continue;
            }
            None => {
                if let Some(marker) = ["```", "~~~"]
                    .into_iter()
                    .find(|marker| trimmed.starts_with(marker))
                {
                    fence = Some(marker);
                    offset += line.len();
                    continue;
                }
            }
        }
        scan_line(line, offset, &mut found);
        offset += line.len();
    }
    found
}

/// Anchors in one line of prose, skipping inline-code spans. A backtick
/// run opens a span that the next run of the same length closes; an
/// unclosed run is not a span, so the rest of the line still scans.
fn scan_line(line: &str, offset: usize, found: &mut Vec<(std::ops::Range<usize>, String)>) {
    let bytes = line.as_bytes();
    let mut at = 0usize;
    while at < bytes.len() {
        if bytes[at] == b'`' {
            let run = bytes[at..].iter().take_while(|b| **b == b'`').count();
            let after = at + run;
            match find_backtick_run(&bytes[after..], run) {
                Some(rel) => at = after + rel + run,
                None => at = after,
            }
            continue;
        }
        if bytes[at] == b'['
            && bytes.get(at + 1) == Some(&b'*')
            && let Some(name) = citation_name(&line[at + 2..])
            && line[at + 2 + name.len()..].starts_with(']')
        {
            let end = at + 2 + name.len() + 1;
            found.push((offset + at..offset + end, name.to_string()));
            at = end;
            continue;
        }
        at += 1;
    }
}

/// Offset of the next backtick run of exactly `len` in `bytes`.
fn find_backtick_run(bytes: &[u8], len: usize) -> Option<usize> {
    let mut at = 0usize;
    while at < bytes.len() {
        if bytes[at] == b'`' {
            let run = bytes[at..].iter().take_while(|b| **b == b'`').count();
            if run == len {
                return Some(at);
            }
            at += run;
            continue;
        }
        at += 1;
    }
    None
}

/// Every anchor in `markdown`, each with the source block it sits in.
/// Blocks are separated by blank lines, which is what the parser does
/// for paragraphs and close enough for list items and table rows.
pub fn citation_anchors(markdown: &str) -> Vec<CitationAnchor> {
    let spans = citation_spans(markdown);
    spans
        .iter()
        .map(|(range, name)| {
            let context = block_context(markdown, range.start, &spans);
            let heading = context.starts_with('#');
            CitationAnchor {
                name: name.clone(),
                context: context.trim_start_matches('#').trim_start().to_string(),
                section: heading,
            }
        })
        .collect()
}

/// The blank-line-delimited block containing `at`, with every anchor
/// removed and whitespace collapsed.
fn block_context(markdown: &str, at: usize, spans: &[(std::ops::Range<usize>, String)]) -> String {
    let start = markdown[..at].rfind("\n\n").map(|i| i + 2).unwrap_or(0);
    let end = markdown[at..]
        .find("\n\n")
        .map(|i| at + i)
        .unwrap_or(markdown.len());
    let mut text = String::with_capacity(end - start);
    let mut cursor = start;
    for (range, _) in spans {
        if range.start < start || range.end > end {
            continue;
        }
        text.push_str(&markdown[cursor..range.start]);
        cursor = range.end;
    }
    text.push_str(&markdown[cursor..end]);
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Rewrite every `[*name]` into its rendered marker: a superscript link
/// that targets itself, so a citation resolves to the exact statement
/// rather than the top of the article. `qualified` composes the full
/// citation string shown on the marker ("Kernel TRM *name").
///
/// The marker carries **no text**: its glyph comes from CSS. An anchor
/// may sit on a heading, and a heading's id and table-of-contents title
/// are built from its text events — a glyph in the markup would leak
/// into both.
pub(crate) fn expand_citation_anchors(markdown: &str, book_short: Option<&str>) -> String {
    let spans = citation_spans(markdown);
    if spans.is_empty() {
        return markdown.to_string();
    }
    let mut out = String::with_capacity(markdown.len() + spans.len() * 96);
    let mut cursor = 0usize;
    for (range, name) in &spans {
        out.push_str(&markdown[cursor..range.start]);
        let full = match book_short {
            Some(short) => format!("{short} *{name}"),
            None => format!("*{name}"),
        };
        out.push_str(&format!(
            "<a class=\"citation-anchor\" id=\"cite-{name}\" href=\"#cite-{name}\" \
             data-citation=\"{full}\" title=\"{full}\" aria-label=\"Citation {full}\">\
             <sup></sup></a>",
            name = escape_attr(name),
            full = escape_attr(&full),
        ));
        cursor = range.end;
    }
    out.push_str(&markdown[cursor..]);
    out
}

fn escape_attr(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    #[test]
    fn persistent_metadata_matches_uncached_with_numbering_and_source_changes() {
        let root = tempfile::tempdir().unwrap();
        let session = crate::cache::Session::new(root.path(), true);
        for source in [
            "## First\n\n[*fixed]\n",
            "## Changed {#custom}\n\n<div id=\"raw\">X</div>",
        ] {
            assert_eq!(
                super::anchor_ids(source),
                super::anchor_ids_uncached(source)
            );
            assert_eq!(
                super::anchor_ids(source),
                super::anchor_ids_uncached(source)
            );
            assert_eq!(
                super::explicit_anchor_ids(source),
                super::explicit_anchor_ids_uncached(source)
            );
            assert_eq!(
                super::explicit_anchor_ids(source),
                super::explicit_anchor_ids_uncached(source)
            );
            for numbering in [None, Some(""), Some("2")] {
                assert_eq!(
                    super::heading_outline(source, numbering),
                    super::heading_outline_uncached(source, numbering)
                );
                assert_eq!(
                    super::heading_outline(source, numbering),
                    super::heading_outline_uncached(source, numbering)
                );
            }
        }
        assert_eq!(session.hits("anchors"), 2);
        assert_eq!(session.hits("explicit-anchors"), 2);
        assert_eq!(session.hits("headings"), 6);
    }
    use super::*;

    /// Render with an empty link index — for tests exercising plain markdown.
    fn render(markdown: &str) -> Rendered {
        super::render(markdown, &LinkIndex::default(), RenderOptions::default()).unwrap()
    }

    fn allow_dangling() -> RenderOptions<'static> {
        RenderOptions {
            allow_dangling: true,
            ..RenderOptions::default()
        }
    }

    fn fixture_index() -> (tempfile::TempDir, LinkIndex) {
        let dir = tempfile::tempdir().unwrap();
        crate::site::write_fixture(dir.path());
        let site = crate::site::Site::load(dir.path(), &dir.path().join("dist")).unwrap();
        let index = LinkIndex::new(&site);
        (dir, index)
    }

    #[test]
    fn tilde_links_resolve_through_the_index() {
        let (_dir, index) = fixture_index();
        let rendered = super::render(
            "See [a2](~alpha/a2#first-section-of-a2).",
            &index,
            RenderOptions::default(),
        )
        .unwrap();
        assert!(
            rendered
                .html
                .contains("<a href=\"/alpha/acorn/wide/a2#first-section-of-a2\">a2</a>")
        );
    }

    #[test]
    fn fragments_are_checked_against_the_target_page() {
        let (_dir, index) = fixture_index();
        let rendered = super::render(
            "See [a2](~alpha/a2#nope).",
            &index,
            RenderOptions::default(),
        )
        .unwrap();
        // The link still renders — one bad anchor must not blank a page —
        // but the build hears about it.
        assert!(rendered.html.contains("href=\"/alpha/acorn/wide/a2#nope\""));
        assert_eq!(rendered.broken.len(), 1, "{:?}", rendered.broken);
        assert!(rendered.broken[0].contains("names anchor '#nope'"));
        assert!(rendered.broken[0].contains("/alpha/acorn/wide/a2"));
        // Downgradable, like a missing page.
        let rendered =
            super::render("See [a2](~alpha/a2#nope).", &index, allow_dangling()).unwrap();
        assert!(rendered.broken.is_empty());
        assert_eq!(rendered.dangling.len(), 1);
        // A page whose anchors the index cannot know — a product page has
        // no body — is not checked.
        let rendered = super::render(
            "See [alpha](~alpha#anything).",
            &index,
            RenderOptions::default(),
        )
        .unwrap();
        assert!(rendered.broken.is_empty());
    }

    #[test]
    fn same_page_fragments_are_checked_against_the_pages_own_ids() {
        let rendered = render(
            "[ok](#real) [cited](#cite-note) [raw](#by-hand) [bad](#missing)\n\n\
             ## Real\n\n[*note] A claim.\n\n<span id=\"by-hand\"></span>\n",
        );
        assert_eq!(rendered.broken.len(), 1, "{:?}", rendered.broken);
        assert!(rendered.broken[0].contains("'#missing' names an anchor this page does not have"));
    }

    #[test]
    fn print_bundles_namespace_same_page_fragments() {
        let rendered = super::render(
            "[up](#overview)\n\n## Overview\n",
            &LinkIndex::default(),
            RenderOptions {
                id_prefix: Some("wide-a1"),
                ..RenderOptions::default()
            },
        )
        .unwrap();
        assert!(rendered.html.contains("href=\"#wide-a1--overview\""));
        assert!(rendered.broken.is_empty(), "{:?}", rendered.broken);
    }

    /// Inspect the emitted HTML independently of the renderer's link checks.
    fn assert_local_hrefs_exist(html: &str) {
        let mut ids = HashSet::new();
        let mut hrefs = Vec::new();
        rewrite_str(
            html,
            RewriteStrSettings {
                element_content_handlers: vec![element!("*", |element| {
                    if let Some(id) = element.get_attribute("id") {
                        ids.insert(html_escape::decode_html_entities(&id).into_owned());
                    }
                    if let Some(href) = element.get_attribute("href") {
                        hrefs.push(html_escape::decode_html_entities(&href).into_owned());
                    }
                    Ok(())
                })],
                ..RewriteStrSettings::default()
            },
        )
        .unwrap();
        for href in hrefs {
            if let Some(fragment) = href.strip_prefix('#').filter(|id| !id.is_empty()) {
                assert!(ids.contains(fragment), "missing target {href}: {html}");
            }
        }
    }

    #[test]
    fn print_namespaces_raw_ids_local_links_and_citation_markers() {
        let body = r##"[dotted](#field.dot) [heading](#field-dot) [block](#block-id)

## <a id="field.dot"></a>`field.dot`

<section
 ID = 'block-id'>
<a HREF = '#field.dot'>raw local</a>
<span id=unquoted></span><a href=#unquoted>unquoted</a>
</section>

[*claim.one] Citation.

[claim](#cite-claim.one)

<a id="article--authored" href="#article--authored">Already looks prefixed</a>
"##;
        let rendered = super::render(
            body,
            &LinkIndex::default(),
            RenderOptions {
                id_prefix: Some("article"),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(rendered.broken.is_empty(), "{:?}", rendered.broken);
        let ids = html_ids(&rendered.html).unwrap();
        for id in [
            "field.dot",
            "field-dot",
            "block-id",
            "unquoted",
            "cite-claim.one",
            "article--authored",
        ] {
            assert!(ids.contains(&format!("article--{id}")), "missing {id}");
            assert!(
                !ids.contains(id),
                "legacy raw aliases must not collide across articles"
            );
        }
        assert_eq!(rendered.toc[0].id, "article--field-dot");
        assert_local_hrefs_exist(&rendered.html);
    }

    #[test]
    fn print_html_rewriting_preserves_comments_raw_text_attributes_and_code() {
        let body = r##"<!-- <a id="comment" href="#comment">comment</a> -->

<script>
const example = '<a id="script" href="#script">';
</script>

<style>
.example::after { content: '<a id="style" href="#style">'; }
</style>

<textarea><a id="textarea" href="#textarea"></textarea>

<span id="real" title='example id="attribute" href="#attribute"'>real</span>

`<a id="inline-code" href="#inline-code">`

```html
<a id="fenced-code" href="#fenced-code">
```

<a href="#real">real</a>
<a href="#">top</a>
<a href="https://example.test/#real">external</a>
<a href="/elsewhere#real">outside</a>
"##;
        let plain = render(body);
        let printed = super::render(
            body,
            &LinkIndex::default(),
            RenderOptions {
                id_prefix: Some("article"),
                ..Default::default()
            },
        )
        .unwrap();
        for unchanged in [
            "<!-- <a id=\"comment\" href=\"#comment\">comment</a> -->",
            "const example = '<a id=\"script\" href=\"#script\">';",
            ".example::after { content: '<a id=\"style\" href=\"#style\">'; }",
            "<textarea><a id=\"textarea\" href=\"#textarea\"></textarea>",
            "title='example id=\"attribute\" href=\"#attribute\"'",
            "href=\"#\"",
            "href=\"https://example.test/#real\"",
            "href=\"/elsewhere#real\"",
        ] {
            assert!(
                printed.html.contains(unchanged),
                "rewrote {unchanged}: {}",
                printed.html
            );
        }
        // Both code forms are escaped/highlighted exactly as on an ordinary page.
        for tag in ["<code>", "<pre"] {
            let start = plain.html.find(tag).unwrap();
            let end_tag = if tag == "<code>" { "</code>" } else { "</pre>" };
            let end = start + plain.html[start..].find(end_tag).unwrap() + end_tag.len();
            assert!(printed.html.contains(&plain.html[start..end]));
        }
        assert_eq!(
            html_ids(&printed.html).unwrap(),
            HashSet::from(["article--real".to_string()])
        );
        assert_local_hrefs_exist(&printed.html);
    }

    #[test]
    fn raw_anchor_entities_are_decoded_once_and_escaped_when_rewritten() {
        let body = r##"<a id='a&amp;b&quot;c' href='&#35;a&amp;b&quot;c'>entity</a>

<a id='literal&amp;amp;' href='#literal&amp;amp;'>literal entity spelling</a>
"##;
        let printed = super::render(
            body,
            &LinkIndex::default(),
            RenderOptions {
                id_prefix: Some("article"),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(printed.html.contains("id=\"article--a&amp;b&quot;c\""));
        assert!(printed.html.contains("href=\"#article--a&amp;b&quot;c\""));
        assert!(
            html_ids(&printed.html)
                .unwrap()
                .contains("article--literal&amp;")
        );
        assert_local_hrefs_exist(&printed.html);
        assert!(anchor_ids(body).contains("a&b\"c"));
    }

    #[test]
    fn standalone_raw_ids_and_fragments_keep_their_authored_spelling() {
        let body = r##"[raw](#field.dot) [heading](#field-dot) [single](#single) [bare](#bare)

## <a id="field.dot"></a>`field.dot`

<span id='single'></span> <span ID=bare></span>

<a href='#field.dot'>raw</a>

[*claim.one] Citation.
"##;
        let plain = render(body);
        assert!(plain.broken.is_empty(), "{:?}", plain.broken);
        for literal in [
            "id=\"field.dot\"",
            "id=\"field-dot\"",
            "id='single'",
            "ID=bare",
            "href='#field.dot'",
            "href=\"#cite-claim.one\"",
        ] {
            assert!(
                plain.html.contains(literal),
                "changed {literal}: {}",
                plain.html
            );
        }
        for id in ["field.dot", "field-dot", "single", "bare", "cite-claim.one"] {
            assert!(anchor_ids(body).contains(id), "source index lacks {id}");
        }
        assert_local_hrefs_exist(&plain.html);
    }

    #[test]
    fn raw_anchor_index_ignores_fake_ids_in_non_element_contexts() {
        let body = r##"<!-- <a id="comment"></a> -->

<script>
const html = '<a id="script"></a>';
</script>

<div
 id='real&amp;one' title=' id="fake"'></div>

`<a id="code"></a>`
"##;
        assert_eq!(anchor_ids(body), HashSet::from(["real&one".to_string()]));
        let rendered = render(&format!("[bad](#comment) [also bad](#script)\n\n{body}"));
        assert_eq!(rendered.broken.len(), 2);
    }

    #[test]
    fn relative_destinations_are_broken_links() {
        let rendered = render(
            "[a](../410--policy.md) [b](sibling.md#part) [c](https://example.com/x.md) \
             [d](/print) [e](mailto:docs@example.com) <docs@example.com>\n\n## Part\n",
        );
        assert_eq!(rendered.broken.len(), 2, "{:?}", rendered.broken);
        assert!(
            rendered
                .html
                .contains("<a href=\"mailto:docs@example.com\">docs@example.com</a>")
        );
        assert!(rendered.broken[0].contains("'../410--policy.md' is a relative path"));
        assert!(rendered.broken[1].contains("'sibling.md#part' is a relative path"));
        let rendered =
            super::render("[a](../x.md)", &LinkIndex::default(), allow_dangling()).unwrap();
        assert!(rendered.broken.is_empty());
        assert_eq!(rendered.dangling.len(), 1);
    }

    #[test]
    fn strict_warns_on_authored_site_absolute_links_without_changing_them() {
        let source = "[root](/) [print](/print) [query](/guide?q=one#part) \
                      [reference][guide]\n\n[guide]: /guide/\n";
        let plain = render(source);
        assert!(plain.warnings.is_empty());
        for allow_dangling in [false, true] {
            let strict = super::render(
                source,
                &LinkIndex::default(),
                RenderOptions {
                    strict: true,
                    allow_dangling,
                    ..RenderOptions::default()
                },
            )
            .unwrap();
            assert_eq!(strict.html, plain.html);
            assert!(strict.broken.is_empty());
            assert!(strict.dangling.is_empty());
            assert_eq!(strict.warnings.len(), 4);
            let destinations = ["/", "/print", "/guide?q=one#part", "/guide/"];
            for (warning, destination) in strict.warnings.iter().zip(destinations) {
                assert!(warning.contains(&format!("link '{destination}' is site-absolute")));
                assert!(warning.contains("~ reference"));
            }
        }
    }

    #[test]
    fn strict_does_not_warn_on_resolved_or_external_links() {
        let (_dir, index) = fixture_index();
        let rendered = super::render(
            "[resolved](~alpha/a2) [external](https://example.com/) \
             [network](//example.com/path) [mail](mailto:docs@example.com) \
             [local](#part) ![image](/image.png)\n\n## Part\n",
            &index,
            RenderOptions {
                strict: true,
                base: "/cache",
                ..RenderOptions::default()
            },
        )
        .unwrap();
        assert!(rendered.warnings.is_empty(), "{:?}", rendered.warnings);
        assert!(rendered.broken.is_empty(), "{:?}", rendered.broken);
        assert!(
            rendered
                .html
                .contains("href=\"/cache/alpha/acorn/wide/a2\"")
        );
        assert!(rendered.html.contains("href=\"//example.com/path\""));
    }

    #[test]
    fn anchor_ids_match_what_render_assigns() {
        let source = "## Overview\n\n## Overview\n\n### When it's hex\n\n\
                      [*claim] Words.\n\n<a id=\"manual\"></a>\n";
        let ids = super::anchor_ids(source);
        for expected in [
            "overview",
            "overview-2",
            "when-it-s-hex",
            "cite-claim",
            "manual",
        ] {
            assert!(ids.contains(expected), "missing {expected}: {ids:?}");
        }
        let html = render(source).html;
        for id in &ids {
            assert!(html.contains(&format!("id=\"{id}\"")), "render lacks {id}");
        }
    }

    #[test]
    fn a_base_moves_the_links_the_tree_owns_and_nothing_else() {
        let (_dir, index) = fixture_index();
        let rendered = super::render(
            "See [a2](~alpha/a2#first-section-of-a2), [the spec](https://example.com/spec) \
             and [the root](/elsewhere).",
            &index,
            RenderOptions {
                base: "/deadbeef",
                ..RenderOptions::default()
            },
        )
        .unwrap();
        // A ~reference names a page in this tree, so it follows the copy.
        assert!(
            rendered
                .html
                .contains("<a href=\"/deadbeef/alpha/acorn/wide/a2#first-section-of-a2\">a2</a>")
        );
        // A URL the author wrote out is emitted exactly as written —
        // prefixing it would break an external link and second-guess a
        // deliberate absolute one.
        assert!(rendered.html.contains("https://example.com/spec"));
        assert!(rendered.html.contains("<a href=\"/elsewhere\">"));
        // And the mirrors move with it.
        let rewritten =
            super::rewrite_source("See [a2](~alpha/a2).\n", &index, None, None, "/deadbeef");
        assert!(rewritten.contains("(/deadbeef/alpha/acorn/wide/a2.md)"));
    }

    #[test]
    fn unresolvable_tilde_links_are_collected_as_broken() {
        let (_dir, index) = fixture_index();
        let rendered = super::render(
            "[gone](~alpha/nope) and [which](~alpha/a1)",
            &index,
            RenderOptions::default(),
        )
        .unwrap();
        // Both problems from one pass, so a build can report them all.
        assert_eq!(rendered.broken.len(), 2);
        assert!(rendered.broken[0].contains("matches no page"));
        assert!(rendered.broken[1].contains("ambiguous"));
        assert!(rendered.dangling.is_empty());
    }

    #[test]
    fn allow_dangling_downgrades_missing_targets_to_warnings() {
        let (_dir, index) = fixture_index();
        let rendered = super::render("[gone](~alpha/nope)", &index, allow_dangling()).unwrap();
        assert!(rendered.html.contains("href=\"~alpha/nope\""));
        assert_eq!(rendered.dangling.len(), 1);
        assert!(rendered.dangling[0].contains("matches no page"));
        assert!(rendered.broken.is_empty());
    }

    #[test]
    fn allow_dangling_still_breaks_on_ambiguous_links() {
        let (_dir, index) = fixture_index();
        let rendered = super::render("[which](~alpha/a1)", &index, allow_dangling()).unwrap();
        assert_eq!(rendered.broken.len(), 1);
        assert!(rendered.broken[0].contains("ambiguous"));
    }

    #[test]
    fn unknown_admonition_types_are_errors() {
        let (_dir, index) = fixture_index();
        let err = super::render(
            "> [!WARN]\n> Mind the gap.\n",
            &index,
            RenderOptions::default(),
        )
        .unwrap_err();
        assert!(
            err.to_string()
                .contains("unknown admonition type '[!WARN]'")
        );
        // Quotes that merely start with brackets stay plain quotes.
        let rendered = render("> [!not-a-marker] just quoting\n");
        assert!(rendered.html.contains("<blockquote>"));
    }

    #[test]
    fn headings_get_ids_and_the_toc_collects_h2_and_h3() {
        let rendered = render("## The String Form\n\ntext\n\n### When it's hex\n\nmore\n");

        assert!(rendered.html.contains("<h2 id=\"the-string-form\">"));
        assert!(rendered.html.contains("<h3 id=\"when-it-s-hex\">"));
        assert_eq!(
            rendered.toc,
            [
                TocEntry {
                    level: 2,
                    id: "the-string-form".into(),
                    title: "The String Form".into(),
                    number: None
                },
                TocEntry {
                    level: 3,
                    id: "when-it-s-hex".into(),
                    title: "When it's hex".into(),
                    number: None
                },
            ]
        );
    }

    #[test]
    fn heading_numbers_continue_the_page_section() {
        let rendered = super::render(
            "## A\n\ntext\n\n### B\n\n#### D\n\n## C\n",
            &LinkIndex::default(),
            RenderOptions {
                numbering: Some("2.1"),
                ..RenderOptions::default()
            },
        )
        .unwrap();

        assert!(
            rendered
                .html
                .contains("<h2 id=\"a\"><span class=\"heading-number\">2.1.1</span> A")
        );
        assert!(
            rendered
                .html
                .contains("<h3 id=\"b\"><span class=\"heading-number\">2.1.1.1</span> B")
        );
        assert!(
            rendered
                .html
                .contains("<h2 id=\"c\"><span class=\"heading-number\">2.1.2</span> C")
        );
        // h4 and deeper stay unnumbered; ids and ToC titles stay clean.
        assert!(rendered.html.contains("<h4 id=\"d\">D"));
        assert_eq!(rendered.toc[0].number.as_deref(), Some("2.1.1"));
        assert_eq!(rendered.toc[0].title, "A");
        assert_eq!(rendered.toc[1].number.as_deref(), Some("2.1.1.1"));
        assert_eq!(rendered.toc[2].number.as_deref(), Some("2.1.2"));
    }

    #[test]
    fn id_prefix_namespaces_heading_anchors() {
        let rendered = super::render(
            "## Overview\n",
            &LinkIndex::default(),
            RenderOptions {
                id_prefix: Some("wide-a1"),
                ..RenderOptions::default()
            },
        )
        .unwrap();
        assert!(rendered.html.contains("<h2 id=\"wide-a1--overview\">"));
    }

    #[test]
    fn rewrite_source_resolves_links_to_mirrors_and_numbers_headings() {
        let (_dir, index) = fixture_index();
        let source = "Intro with [a2](~alpha/a2#first-section-of-a2) and [missing](~alpha/nope).\n\n\
                      ## First\n\n```\n## not a heading [x](~alpha/a2)\n```\n\n### Sub\n";
        let rewritten = super::rewrite_source(source, &index, Some("2.1"), None, "");
        assert!(rewritten.contains("[a2](/alpha/acorn/wide/a2.md#first-section-of-a2)"));
        // Unresolvable links and code-block contents stay untouched.
        assert!(rewritten.contains("[missing](~alpha/nope)"));
        assert!(rewritten.contains("## not a heading [x](~alpha/a2)"));
        assert!(rewritten.contains("## 2.1.1 First"));
        assert!(rewritten.contains("### 2.1.1.1 Sub"));
    }

    #[test]
    fn rewrite_source_without_numbering_leaves_headings_alone() {
        let (_dir, index) = fixture_index();
        let rewritten =
            super::rewrite_source("## Plain\n\nSee [a2](~alpha/a2).\n", &index, None, None, "");
        assert!(rewritten.contains("## Plain"));
        assert!(rewritten.contains("[a2](/alpha/acorn/wide/a2.md)"));
    }

    #[test]
    fn duplicate_heading_ids_are_deduplicated() {
        let rendered = render("## Overview\n\n## Overview\n");
        assert!(rendered.html.contains("id=\"overview\""));
        assert!(rendered.html.contains("id=\"overview-2\""));
    }

    #[test]
    fn h4_and_deeper_stay_out_of_the_toc() {
        let rendered = render("#### Deep detail\n");
        assert!(rendered.html.contains("<h4 id=\"deep-detail\">"));
        assert!(rendered.toc.is_empty());
    }

    #[test]
    fn mermaid_fences_become_mermaid_pre_blocks() {
        let rendered = render("```mermaid\ngraph TD;\n  A-->B;\n```\n");
        assert!(rendered.has_mermaid);
        assert!(
            rendered
                .html
                .contains("<pre class=\"mermaid\">graph TD;\n  A--&gt;B;\n</pre>")
        );
        assert!(!rendered.html.contains("language-mermaid"));
    }

    #[test]
    fn math_fences_and_dollar_code_spans_render_as_mathml() {
        let rendered = render("Where $`N`$ costs $5:\n\n```math\n\\frac{1}{2^N}\n```\n");
        assert!(rendered.broken.is_empty(), "{:?}", rendered.broken);
        assert!(
            rendered.html.contains("<p>Where <math display=\"inline\">"),
            "{}",
            rendered.html
        );
        assert!(rendered.html.contains(" costs $5:</p>"));
        assert!(
            rendered
                .html
                .contains("<div class=\"math-display\"><math display=\"block\">")
        );
        assert!(!rendered.html.contains("code-block"));

        let rendered = render("```math\n\\frac{1}\n```\n");
        assert_eq!(rendered.broken.len(), 1);
        assert!(rendered.broken[0].contains("equation '\\frac{1}' is not valid TeX"));
    }

    #[test]
    fn ordinary_code_blocks_are_highlighted_and_copyable() {
        let rendered = render("```rust\nfn main() {}\n```\n");
        assert!(!rendered.has_mermaid);
        assert!(
            rendered
                .html
                .contains("<div class=\"code-block\" data-language=\"rust\">")
        );
        assert!(rendered.html.contains("data-copy-code"));
        assert!(rendered.html.contains("hl-storage"));
        // Indented blocks get the same chrome, without a language.
        let rendered = render("    plain text\n");
        assert!(rendered.html.contains("<div class=\"code-block\">"));
        assert!(!rendered.html.contains("data-language"));
    }

    #[test]
    fn headings_carry_permalink_anchors() {
        let rendered = render("## The Rule\n");
        assert!(rendered.html.contains(
            "<h2 id=\"the-rule\">The Rule \
             <a class=\"heading-anchor\" href=\"#the-rule\" \
             aria-label=\"Link to this section\">#</a></h2>"
        ));
        // The ToC title stays clean of it.
        assert_eq!(rendered.toc[0].title, "The Rule");
    }

    #[test]
    fn admonitions_render_as_titled_asides() {
        let rendered = render("> [!WARNING]\n> Mind the gap.\n");
        assert!(
            rendered
                .html
                .contains("<aside class=\"admonition admonition-warning\">")
        );
        assert!(
            rendered
                .html
                .contains("<p class=\"admonition-title\">Warning</p>")
        );
        assert!(rendered.html.contains("Mind the gap."));
        assert!(rendered.html.contains("</aside>"));
        assert!(!rendered.html.contains("<blockquote>"));
    }

    #[test]
    fn plain_blockquotes_stay_blockquotes() {
        let rendered = render("> just quoting someone\n");
        assert!(rendered.html.contains("<blockquote>"));
        assert!(!rendered.html.contains("admonition"));
    }

    /// A one-image index: wiring.png (the 2×1 test PNG) inside "topic",
    /// published at /alpha/topic/wiring.png.
    fn image_fixture() -> (tempfile::TempDir, crate::images::ImageIndex) {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("topic")).unwrap();
        std::fs::write(dir.path().join("topic/wiring.png"), crate::site::TEST_PNG).unwrap();
        let index = crate::images::ImageIndex::new(&[crate::images::ImageAsset {
            source: dir.path().join("topic/wiring.png"),
            url: "/alpha/topic/wiring.png".into(),
        }]);
        (dir, index)
    }

    #[test]
    fn relative_images_resolve_with_dimensions() {
        let (dir, index) = image_fixture();
        let topic = dir.path().join("topic");
        let rendered = super::render(
            "Inline ![Alt \"text\"](wiring.png) here.",
            &LinkIndex::default(),
            RenderOptions {
                images: Some(ImageScope {
                    index: &index,
                    dir: &topic,
                }),
                ..RenderOptions::default()
            },
        )
        .unwrap();
        assert!(rendered.html.contains(
            "<img src=\"/alpha/topic/wiring.png\" \
             alt=\"Alt &quot;text&quot;\" width=\"2\" height=\"1\">"
        ));
        assert!(rendered.broken.is_empty());
    }

    #[test]
    fn captioned_standalone_images_become_figures() {
        let (dir, index) = image_fixture();
        let topic = dir.path().join("topic");
        let options = RenderOptions {
            images: Some(ImageScope {
                index: &index,
                dir: &topic,
            }),
            ..RenderOptions::default()
        };
        let rendered = super::render(
            "![Alt](wiring.png \"A caption\")\n",
            &LinkIndex::default(),
            options,
        )
        .unwrap();
        assert_eq!(
            rendered.html,
            "<figure><img src=\"/alpha/topic/wiring.png\" alt=\"Alt\" \
             width=\"2\" height=\"1\"><figcaption>A caption</figcaption></figure>\n"
        );

        // Mid-sentence, the caption stays a plain title attribute — an
        // <img> inside a <p> can't legally hold a <figcaption>.
        let rendered = super::render(
            "Before ![Alt](wiring.png \"Cap\") after.",
            &LinkIndex::default(),
            options,
        )
        .unwrap();
        assert!(rendered.html.contains("<p>Before <img"));
        assert!(rendered.html.contains(" title=\"Cap\"> after.</p>"));
        assert!(!rendered.html.contains("<figure>"));
    }

    #[test]
    fn missing_images_are_broken_and_downgradable() {
        let (dir, index) = image_fixture();
        let topic = dir.path().join("topic");
        let scope = Some(ImageScope {
            index: &index,
            dir: &topic,
        });
        let rendered = super::render(
            "![x](gone.png)",
            &LinkIndex::default(),
            RenderOptions {
                images: scope,
                ..RenderOptions::default()
            },
        )
        .unwrap();
        assert_eq!(rendered.broken.len(), 1);
        assert!(rendered.broken[0].contains("image 'gone.png' not found"));

        let rendered = super::render(
            "![x](gone.png)",
            &LinkIndex::default(),
            RenderOptions {
                images: scope,
                allow_dangling: true,
                ..RenderOptions::default()
            },
        )
        .unwrap();
        assert!(rendered.broken.is_empty());
        assert_eq!(rendered.dangling.len(), 1);
        assert!(rendered.html.contains("src=\"gone.png\""));
    }

    #[test]
    fn tilde_image_destinations_are_always_broken() {
        let rendered =
            super::render("![x](~alpha/a2)", &LinkIndex::default(), allow_dangling()).unwrap();
        assert_eq!(rendered.broken.len(), 1);
        assert!(rendered.broken[0].contains("~references name pages"));
    }

    #[test]
    fn external_and_absolute_images_pass_through() {
        let rendered = render("![x](https://example.com/i.png) and ![y](/assets/logo.svg)\n");
        assert!(rendered.html.contains("src=\"https://example.com/i.png\""));
        assert!(rendered.html.contains("src=\"/assets/logo.svg\""));
        assert!(rendered.broken.is_empty());
    }

    #[test]
    fn rewrite_source_rewrites_image_destinations() {
        let (dir, index) = image_fixture();
        let topic = dir.path().join("topic");
        let scope = Some(ImageScope {
            index: &index,
            dir: &topic,
        });
        let source = "![Alt](wiring.png \"Cap\")\n\n![B](wiring.png)\n\n![C](gone.png)\n";
        let rewritten = super::rewrite_source(source, &LinkIndex::default(), None, scope, "");
        assert!(rewritten.contains("![Alt](/alpha/topic/wiring.png \"Cap\")"));
        assert!(rewritten.contains("![B](/alpha/topic/wiring.png)"));
        // Unresolvable destinations stay untouched, like links.
        assert!(rewritten.contains("![C](gone.png)"));
    }

    #[test]
    fn rfc_keywords_highlight_in_prose_but_not_code() {
        let rendered = render(
            "You MUST NOT fail, though you MAY retry.\n\n```\nMUST\n```\n\nUse `MUST` here. A MUSTARD note.\n",
        );
        assert!(
            rendered
                .html
                .contains("<span class=\"rfc-keyword\">MUST NOT</span>")
        );
        assert!(
            rendered
                .html
                .contains("<span class=\"rfc-keyword\">MAY</span>")
        );
        // Code (fenced and inline) and ordinary words stay untouched.
        assert!(rendered.html.contains("<code>MUST\n</code>"));
        assert!(rendered.html.contains("<code>MUST</code>"));
        assert!(rendered.html.contains("MUSTARD"));
        assert_eq!(rendered.html.matches("rfc-keyword").count(), 2);
    }

    #[test]
    fn rfc_keywords_stay_out_of_headings_and_links() {
        let rendered = render("## What You MUST Do\n\n[MUST read](https://example.com/)\n");
        assert!(!rendered.html.contains("rfc-keyword"));
        assert!(rendered.html.contains("<h2 id=\"what-you-must-do\">"));
    }

    #[test]
    fn heading_outline_matches_what_render_assigns() {
        let source = "## A\n\ntext\n\n### B\n\n#### D\n\n## A\n";
        let outline = heading_outline(source, Some("2.1"));
        let rendered = super::render(
            source,
            &LinkIndex::default(),
            RenderOptions {
                numbering: Some("2.1"),
                ..RenderOptions::default()
            },
        )
        .unwrap();
        assert_eq!(outline, rendered.toc);
        assert_eq!(outline[2].id, "a-2");
        assert_eq!(outline[2].number.as_deref(), Some("2.1.2"));
    }

    #[test]
    fn citation_names_are_lowercase_dotted_segments() {
        assert_eq!(
            citation_name("copy-up.preserves-ownership]"),
            Some("copy-up.preserves-ownership")
        );
        assert_eq!(citation_name("a1]"), Some("a1"));
        // Must start and end alphanumeric, and carry no doubled or
        // adjacent separators.
        assert_eq!(citation_name(".leading]"), None);
        assert_eq!(citation_name("trailing.]"), None);
        assert_eq!(citation_name("double..dot]"), None);
        assert_eq!(citation_name("mixed-.sep]"), None);
        // Uppercase is rejected rather than folded: one spelling only.
        assert_eq!(citation_name("Upper]"), None);
        assert_eq!(citation_name("]"), None);
    }

    #[test]
    fn citation_anchors_survive_emphasis_in_the_same_block() {
        // The hazard the pre-parse pass exists for: the `*` inside
        // `[*name]` is left-flanking, so an emphasis `*` later in the
        // block could close it and swallow the anchor into an <em>.
        let body = "Ownership is preserved. [*copy-up.owner] And *emphasis* follows.\n";
        let anchors = citation_anchors(body);
        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].name, "copy-up.owner");

        let html = render(body).html;
        assert!(html.contains("id=\"cite-copy-up.owner\""), "{html}");
        // The emphasis still renders as emphasis, and no stray asterisk
        // or raw marker survives into the output.
        assert!(html.contains("<em>emphasis</em>"), "{html}");
        assert!(!html.contains("[*copy-up.owner]"), "{html}");
    }

    #[test]
    fn citation_anchors_skip_code_fences_and_inline_code() {
        let body = "Prose. [*real.anchor]\n\n\
                    Inline `[*inline.example]` stays literal.\n\n\
                    ```\n[*fenced.example]\n```\n";
        let names: Vec<_> = citation_anchors(body).into_iter().map(|a| a.name).collect();
        assert_eq!(names, vec!["real.anchor"]);
    }

    #[test]
    fn an_anchor_on_a_heading_names_the_section() {
        let body = "## The provider [*resolution.provider]\n\nProse below.\n";
        let anchors = citation_anchors(body);
        assert_eq!(anchors.len(), 1);
        assert!(
            anchors[0].section,
            "heading anchor should be a section anchor"
        );
        // The `#` markers are not part of the section's name-in-prose.
        assert_eq!(anchors[0].context, "The provider");

        // The heading's own id and ToC title stay clean: the marker is
        // inline HTML by the time the parser sees it, so it never
        // reaches the slug.
        let rendered = render(body);
        assert!(
            rendered.html.contains("<h2 id=\"the-provider\""),
            "{}",
            rendered.html
        );
        assert_eq!(rendered.toc[0].title, "The provider ");

        // An anchor in ordinary prose is not a section anchor.
        assert!(!citation_anchors("A rule. [*a.rule]\n")[0].section);
    }

    #[test]
    fn citation_context_is_the_block_with_markers_removed() {
        let body = "First block.\n\n\
                    Ownership [*one] is preserved\nacross lines. [*two]\n\n\
                    Third block.\n";
        let anchors = citation_anchors(body);
        assert_eq!(anchors.len(), 2);
        // Both anchors in one block share its context, and neither the
        // markers nor the line break survive into it.
        assert_eq!(anchors[0].context, "Ownership is preserved across lines.");
        assert_eq!(anchors[0].context, anchors[1].context);
    }

    #[test]
    fn a_citation_marker_renders_as_a_self_targeting_superscript() {
        let body = "Ownership is preserved. [*copy-up.owner]\n";
        let html = render(body).html;
        // Self-targeting so a citation resolves to the statement, not
        // the top of the article.
        assert!(html.contains("href=\"#cite-copy-up.owner\""), "{html}");
        assert!(html.contains("<sup>"), "{html}");
    }

    #[test]
    fn tab_groups_expand_and_their_bodies_stay_markdown() {
        let rendered = render(
            ":::tabs\n:::tab npm\nRun `npm i` — see **docs**.\n\n             :::tab pnpm\n```sh\npnpm add x\n```\n:::\n\nAfter.\n",
        );
        assert!(rendered.html.contains("<div class=\"tabs\" data-tabs>"));
        assert!(rendered.html.contains(
            "<button class=\"tab-button\" type=\"button\" role=\"tab\" \
             data-tab=\"0\" aria-selected=\"true\">npm</button>"
        ));
        assert!(
            rendered
                .html
                .contains("aria-selected=\"false\">pnpm</button>")
        );
        // Only the first panel is visible.
        assert!(rendered.html.contains("data-tab-panel=\"0\">"));
        assert!(rendered.html.contains("data-tab-panel=\"1\" hidden>"));
        // Panel bodies parse as markdown, code fences included.
        assert!(rendered.html.contains("<strong>docs</strong>"));
        assert!(rendered.html.contains("<code>npm i</code>"));
        assert!(rendered.html.contains("data-language=\"sh\""));
        assert!(rendered.html.contains("<p>After.</p>"));
    }

    #[test]
    fn tab_markers_inside_code_samples_stay_literal() {
        let rendered = render("```markdown\n:::tabs\n:::tab One\n:::\n```\n");
        assert!(!rendered.html.contains("<div class=\"tabs\""));
        assert!(rendered.html.contains(":::tabs"));
    }

    #[test]
    fn malformed_tab_groups_are_errors() {
        let cases = [
            (":::tabs\n:::tab One\ntext\n", "unclosed"),
            (":::tab One\ntext\n:::\n", "outside a ':::tabs' block"),
            (":::tabs\n:::\n", "no ':::tab' labels"),
            (":::tabs\n:::tab \ntext\n:::\n", "needs a label"),
            (":::tabs\n:::tab One\n:::tabs\n:::\n:::\n", "cannot nest"),
            ("text\n:::\n", "none is open"),
        ];
        for (source, expected) in cases {
            let error = super::render(source, &LinkIndex::default(), RenderOptions::default())
                .unwrap_err()
                .to_string();
            assert!(
                error.contains(expected),
                "{source:?} gave {error:?}, wanted {expected:?}"
            );
        }
    }

    #[test]
    fn tables_and_inline_code_render() {
        let rendered = render("| a | b |\n|---|---|\n| 1 | 2 |\n\nUse `token` here.\n");
        // Tables sit inside their own scroll container, so a wide one
        // never forces the page to scroll sideways.
        assert!(
            rendered
                .html
                .contains("<div class=\"table-scroll\">\n<table>")
        );
        assert!(rendered.html.contains("</table>\n</div>"));
        assert!(rendered.html.contains("<code>token</code>"));
    }
}
