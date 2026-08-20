use std::collections::HashMap;
use std::fmt::Write as _;

use anyhow::{Result, bail};
use pulldown_cmark::{BlockQuoteKind, CodeBlockKind, Event, Options, Parser, Tag, TagEnd, html};
use serde::Serialize;

use crate::images::{ImageInfo, ImageScope};
use crate::links::{LinkIndex, ResolveError};

/// One "On this page" entry: an h2 or h3 with its generated anchor id.
#[derive(Debug, PartialEq, Serialize)]
pub struct TocEntry {
    pub level: u8,
    pub id: String,
    pub title: String,
    /// The heading's section number ("2.1.3") when the page is numbered.
    pub number: Option<String>,
}

#[derive(Debug)]
pub struct Rendered {
    pub html: String,
    pub toc: Vec<TocEntry>,
    /// Whether the article contains a mermaid diagram, so its page can pull
    /// in the (otherwise omitted) mermaid script.
    pub has_mermaid: bool,
    /// Dangling-link warnings — missing targets when `allow_dangling`
    /// downgrades them.
    pub dangling: Vec<String>,
    /// Fatal link problems: ambiguous references (always), and missing
    /// targets when they aren't allowed. Collected rather than failing
    /// fast so one build reports every broken link at once; the caller
    /// decides when to abort.
    pub broken: Vec<String>,
}

/// How one article renders; see `render`.
#[derive(Debug, Default, Clone, Copy)]
pub struct RenderOptions<'a> {
    /// Downgrade missing ~link targets from render errors to collected
    /// warnings (ambiguous links always fail — the author has to pick).
    pub allow_dangling: bool,
    /// The page's section number inside a book ("2.1"): h2/h3 headings
    /// continue the dotted numbering as real text — searchable and
    /// copyable, unlike CSS counters — while ids and ToC titles stay
    /// clean of it.
    pub numbering: Option<&'a str>,
    /// Namespace for heading anchor ids, for pages that concatenate many
    /// articles (print pages) where slugs would otherwise collide.
    pub id_prefix: Option<&'a str>,
    /// Where relative `![](...)` destinations resolve: the site's image
    /// index plus this article's source directory. None (plain-markdown
    /// tests) leaves image events untouched.
    pub images: Option<ImageScope<'a>>,
}

/// Render article markdown to HTML. Every heading gets a slugified,
/// deduplicated anchor id; h2/h3 headings are also collected into the ToC.
/// ```` ```mermaid ```` fences become `<pre class="mermaid">` blocks for
/// client-side mermaid to render. `~` link destinations are resolved
/// through the link index.
pub fn render(markdown: &str, links: &LinkIndex, options: RenderOptions) -> Result<Rendered> {
    let events: Vec<Event> = Parser::new_ext(markdown, parser_options()).collect();

    let mut out = Vec::with_capacity(events.len());
    let mut toc = Vec::new();
    let mut used_ids = HashMap::new();
    let mut has_mermaid = false;
    let mut dangling = Vec::new();
    let mut broken = Vec::new();
    // Tracks open blockquotes: true = admonition we opened as an <aside>.
    let mut blockquotes = Vec::new();
    // Section-number counters, used only when `numbering` is set.
    let (mut h2_count, mut h3_count) = (0, 0);

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
            let mut title = String::new();
            for event in &events[i + 1..] {
                match event {
                    Event::End(TagEnd::Heading(_)) => break,
                    Event::Text(text) | Event::Code(text) => title.push_str(text),
                    _ => {}
                }
            }
            let mut base = match id {
                Some(explicit) => explicit.to_string(),
                None => slugify(&title),
            };
            if let Some(prefix) = options.id_prefix {
                base = format!("{prefix}--{base}");
            }
            let unique = unique_id(&mut used_ids, base);
            let level_number = *level as u8;
            let number = match (options.numbering, level_number) {
                (Some(prefix), 2) => {
                    h2_count += 1;
                    h3_count = 0;
                    Some(format!("{prefix}.{h2_count}"))
                }
                (Some(prefix), 3) => {
                    h3_count += 1;
                    Some(format!("{prefix}.{h2_count}.{h3_count}"))
                }
                _ => None,
            };
            if (2..=3).contains(&level_number) {
                toc.push(TocEntry {
                    level: level_number,
                    id: unique.clone(),
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
        } else if let Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) = &events[i]
            && let Some(reference) = dest_url.strip_prefix('~')
        {
            let (target, fragment) = match reference.split_once('#') {
                Some((target, fragment)) => (target, Some(fragment)),
                None => (reference, None),
            };
            match links.resolve(target) {
                Ok(mut resolved) => {
                    if let Some(fragment) = fragment {
                        resolved = format!("{resolved}#{fragment}");
                    }
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
                ImageFate::PassThrough => out.push(events[i].clone()),
                ImageFate::Broken(message) => {
                    broken.push(message);
                    out.push(events[i].clone());
                }
                ImageFate::Dangling(message) => {
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
        } else {
            out.push(events[i].clone());
        }
        i += 1;
    }

    let mut rendered = String::new();
    html::push_html(&mut rendered, out.into_iter());
    Ok(Rendered {
        html: rendered,
        toc,
        has_mermaid,
        dangling,
        broken,
    })
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
        Some(info) => ImageFate::Local(info.clone()),
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
fn parser_options() -> Options {
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
) -> String {
    let mut edits: Vec<(std::ops::Range<usize>, String)> = Vec::new();
    let (mut h2_count, mut h3_count) = (0, 0);
    for (event, range) in Parser::new_ext(markdown, parser_options()).into_offset_iter() {
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
                let new_dest = match fragment {
                    Some(fragment) => format!("{resolved}.md#{fragment}"),
                    None => format!("{resolved}.md"),
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
                    edits.push((dest_range, info.url.clone()));
                }
            }
            Event::Start(Tag::Heading { level, .. }) if numbering.is_some() => {
                let number = match (numbering, level as u8) {
                    (Some(prefix), 2) => {
                        h2_count += 1;
                        h3_count = 0;
                        Some(format!("{prefix}.{h2_count}"))
                    }
                    (Some(prefix), 3) => {
                        h3_count += 1;
                        Some(format!("{prefix}.{h2_count}.{h3_count}"))
                    }
                    _ => None,
                };
                let Some(number) = number else { continue };
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
fn escape_text(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escape text for an HTML attribute value (double-quoted).
fn escape_attribute(text: &str) -> String {
    escape_text(text).replace('"', "&quot;")
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

#[cfg(test)]
mod tests {
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
            "See [a2](~alpha/a2#part).",
            &index,
            RenderOptions::default(),
        )
        .unwrap();
        assert!(
            rendered
                .html
                .contains("<a href=\"/alpha/acorn/wide/a2#part\">a2</a>")
        );
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
                .contains("<h2 id=\"a\"><span class=\"heading-number\">2.1.1</span> A</h2>")
        );
        assert!(
            rendered
                .html
                .contains("<h3 id=\"b\"><span class=\"heading-number\">2.1.1.1</span> B</h3>")
        );
        assert!(
            rendered
                .html
                .contains("<h2 id=\"c\"><span class=\"heading-number\">2.1.2</span> C</h2>")
        );
        // h4 and deeper stay unnumbered; ids and ToC titles stay clean.
        assert!(rendered.html.contains("<h4 id=\"d\">D</h4>"));
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
        let source = "Intro with [a2](~alpha/a2#part) and [missing](~alpha/nope).\n\n\
                      ## First\n\n```\n## not a heading [x](~alpha/a2)\n```\n\n### Sub\n";
        let rewritten = super::rewrite_source(source, &index, Some("2.1"), None);
        assert!(rewritten.contains("[a2](/alpha/acorn/wide/a2.md#part)"));
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
            super::rewrite_source("## Plain\n\nSee [a2](~alpha/a2).\n", &index, None, None);
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
    fn ordinary_code_blocks_do_not_flag_mermaid() {
        let rendered = render("```rust\nfn main() {}\n```\n");
        assert!(!rendered.has_mermaid);
        assert!(rendered.html.contains("language-rust"));
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
        let rewritten = super::rewrite_source(source, &LinkIndex::default(), None, scope);
        assert!(rewritten.contains("![Alt](/alpha/topic/wiring.png \"Cap\")"));
        assert!(rewritten.contains("![B](/alpha/topic/wiring.png)"));
        // Unresolvable destinations stay untouched, like links.
        assert!(rewritten.contains("![C](gone.png)"));
    }

    #[test]
    fn tables_and_inline_code_render() {
        let rendered = render("| a | b |\n|---|---|\n| 1 | 2 |\n\nUse `token` here.\n");
        assert!(rendered.html.contains("<table>"));
        assert!(rendered.html.contains("<code>token</code>"));
    }
}
