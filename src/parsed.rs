//! Parse each distinct Markdown source once, sharing it across article,
//! anchor and print passes. Render-specific rewriting stays in markdown.rs.
use crate::memo::{Memo, key};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};
use std::{cell::RefCell, ops::Range, sync::Arc};

type Events = Arc<Vec<(Event<'static>, Range<usize>)>>;

/// Borrow large text payloads while a parsed article is in use. Cloning
/// owned CowStr values would allocate the article's text again on every
/// print pass, defeating much of the benefit of sharing its events.
pub fn borrow_event<'a>(event: &'a Event<'static>) -> Event<'a> {
    match event {
        Event::Text(s) => Event::Text(s.as_ref().into()),
        Event::Code(s) => Event::Code(s.as_ref().into()),
        Event::InlineMath(s) => Event::InlineMath(s.as_ref().into()),
        Event::DisplayMath(s) => Event::DisplayMath(s.as_ref().into()),
        Event::Html(s) => Event::Html(s.as_ref().into()),
        Event::InlineHtml(s) => Event::InlineHtml(s.as_ref().into()),
        Event::FootnoteReference(s) => Event::FootnoteReference(s.as_ref().into()),
        _ => event.clone(),
    }
}

pub fn parse(source: &str, options: Options) -> Events {
    thread_local! {
        static CACHE: RefCell<Memo<Events>> = RefCell::new(Memo::new(256 * 1024 * 1024));
    }
    let key = key(&[source.as_bytes(), &options.bits().to_le_bytes()]);
    if crate::cache::enabled()
        && let Some(events) = CACHE.with(|cache| cache.borrow().get(&key))
    {
        return events;
    }
    let events: Vec<_> = Parser::new_ext(source, options)
        .into_offset_iter()
        .map(|(event, range)| (event.into_static(), range))
        .collect();
    let bytes = events.capacity() * std::mem::size_of::<(Event, Range<usize>)>()
        + events
            .iter()
            .map(|(event, _)| heap_bytes(event))
            .sum::<usize>();
    let events = Arc::new(events);
    if crate::cache::enabled() {
        CACHE.with(|cache| cache.borrow_mut().insert(key, events.clone(), bytes));
    }
    events
}

fn heap_bytes(event: &Event) -> usize {
    // Include string allocation overhead, even for inline/empty CowStrs.
    let string = |s: &str| s.len() + 32;
    match event {
        Event::Text(s)
        | Event::Code(s)
        | Event::InlineMath(s)
        | Event::DisplayMath(s)
        | Event::Html(s)
        | Event::InlineHtml(s)
        | Event::FootnoteReference(s) => string(s),
        Event::Start(tag) => match tag {
            Tag::Link {
                dest_url,
                title,
                id,
                ..
            }
            | Tag::Image {
                dest_url,
                title,
                id,
                ..
            } => string(dest_url) + string(title) + string(id),
            Tag::CodeBlock(CodeBlockKind::Fenced(s)) | Tag::FootnoteDefinition(s) => string(s),
            Tag::Heading {
                id, classes, attrs, ..
            } => {
                id.as_ref().map_or(0, |s| string(s))
                    + classes.capacity() * std::mem::size_of::<pulldown_cmark::CowStr>()
                    + classes.iter().map(|s| string(s)).sum::<usize>()
                    + attrs.capacity()
                        * std::mem::size_of::<(
                            pulldown_cmark::CowStr,
                            Option<pulldown_cmark::CowStr>,
                        )>()
                    + attrs
                        .iter()
                        .map(|(k, v)| string(k) + v.as_ref().map_or(0, |s| string(s)))
                        .sum::<usize>()
            }
            Tag::Table(alignments) => {
                alignments.capacity() * std::mem::size_of::<pulldown_cmark::Alignment>()
            }
            _ => 0,
        },
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cached_events_preserve_offsets_and_parser_options() {
        let source = "# Heading {#explicit}\n\n[ref][x] and ~~strike~~ café\n\n[x]: /target\n\n```rust\nlet x = 1;\n```\n";
        for options in [Options::empty(), Options::all()] {
            let expected: Vec<_> = Parser::new_ext(source, options)
                .into_offset_iter()
                .map(|(e, r)| (e.into_static(), r))
                .collect();
            let first = parse(source, options);
            assert_eq!(*first, expected);
            assert_eq!(
                first
                    .iter()
                    .map(|(event, range)| (borrow_event(event), range.clone()))
                    .collect::<Vec<_>>(),
                expected
            );
            assert!(Arc::ptr_eq(&first, &parse(source, options)));
        }
        assert!(!Arc::ptr_eq(
            &parse(source, Options::empty()),
            &parse("changed", Options::empty())
        ));
    }
}
