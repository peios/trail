//! Line breaks that keep what belongs together on one line, in rendered
//! output only: sources stay plain, since non-breaking characters there
//! search and diff differently from what they look like.
//!
//! - A spaced em dash ("the kernel — not the service") never starts a
//!   line: the space before it becomes a non-breaking one.
//! - A numeric range ("0–65535") never splits after its en dash.
//! - A key combination (<kbd>Ctrl</kbd>+<kbd>C</kbd>) never splits at
//!   its "+".
//!
//! Ranges and key combinations are wrapped in `<span class="keep-together">`
//! rather than joined with an invisible character, which would ride along
//! into anything a reader copies.

use pulldown_cmark::{CowStr, Event, Tag, TagEnd};

const NO_BREAK_SPACE: char = '\u{a0}';
const OPEN: &str = "<span class=\"keep-together\">";
const CLOSE: &str = "</span>";

/// Apply the rules to a parsed body. Code blocks are left exactly as
/// written; code spans are separate events and never touched.
pub fn keep_together(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
    let mut out = Vec::with_capacity(events.len());
    let mut in_code_block = false;
    let mut events = events.into_iter().peekable();
    while let Some(event) = events.next() {
        match event {
            Event::Start(Tag::CodeBlock(_)) => {
                in_code_block = true;
                out.push(event);
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                out.push(event);
            }
            Event::Text(text) if !in_code_block => {
                let text = text.replace(" \u{2014}", "\u{a0}\u{2014}");
                push_with_ranges(&mut out, &text);
            }
            // A line break in the source just before a dash.
            Event::SoftBreak if matches!(events.peek(), Some(Event::Text(next)) if next.starts_with('\u{2014}')) =>
            {
                out.push(Event::Text(NO_BREAK_SPACE.to_string().into()));
            }
            event => out.push(event),
        }
    }
    join_key_combinations(out)
}

/// Push `text`, wrapping each word that holds a digit–en dash–digit range.
fn push_with_ranges<'a>(out: &mut Vec<Event<'a>>, text: &str) {
    let mut rest = text;
    while let Some((start, end)) = next_range(rest) {
        if start > 0 {
            out.push(Event::Text(CowStr::from(rest[..start].to_string())));
        }
        out.push(Event::InlineHtml(OPEN.into()));
        out.push(Event::Text(CowStr::from(rest[start..end].to_string())));
        out.push(Event::InlineHtml(CLOSE.into()));
        rest = &rest[end..];
    }
    if !rest.is_empty() {
        out.push(Event::Text(CowStr::from(rest.to_string())));
    }
}

/// The byte range of the first whitespace-delimited word in `text` that
/// has an en dash between two digits.
fn next_range(text: &str) -> Option<(usize, usize)> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    for (i, &(at, c)) in chars.iter().enumerate() {
        let joined = c == '\u{2013}'
            && i > 0
            && chars[i - 1].1.is_ascii_digit()
            && chars
                .get(i + 1)
                .is_some_and(|(_, next)| next.is_ascii_digit());
        if !joined {
            continue;
        }
        let start = text[..at].rfind(char::is_whitespace).map_or(0, |space| {
            space + text[space..].chars().next().map_or(1, char::len_utf8)
        });
        let end = text[at..]
            .find(char::is_whitespace)
            .map_or(text.len(), |space| at + space);
        return Some((start, end));
    }
    None
}

/// Wrap each run of `<kbd>…</kbd>` joined by "+" in a keep-together span.
fn join_key_combinations(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
    let mut out = Vec::with_capacity(events.len());
    let mut i = 0;
    while i < events.len() {
        let end = combination_end(&events, i);
        if end > i {
            out.push(Event::InlineHtml(OPEN.into()));
            out.extend(events[i..end].iter().cloned());
            out.push(Event::InlineHtml(CLOSE.into()));
            i = end;
        } else {
            out.push(events[i].clone());
            i += 1;
        }
    }
    out
}

/// If a key combination starts at `start`, the index just past it;
/// otherwise `start`. A combination is two or more `<kbd>` elements, each
/// followed by a lone "+" text before the next.
fn combination_end(events: &[Event<'_>], start: usize) -> usize {
    let mut keys = 0;
    let mut i = start;
    let mut end = start;
    while let Some(after) = kbd_element(events, i) {
        keys += 1;
        end = after;
        match (events.get(after), events.get(after + 1)) {
            (Some(Event::Text(plus)), Some(Event::InlineHtml(next)))
                if plus.as_ref() == "+" && next.as_ref() == "<kbd>" =>
            {
                i = after + 1;
            }
            _ => break,
        }
    }
    if keys >= 2 { end } else { start }
}

/// If `<kbd>`, text, `</kbd>` starts at `i`, the index just past it.
fn kbd_element(events: &[Event<'_>], i: usize) -> Option<usize> {
    match (events.get(i), events.get(i + 1), events.get(i + 2)) {
        (Some(Event::InlineHtml(open)), Some(Event::Text(_)), Some(Event::InlineHtml(close)))
            if open.as_ref() == "<kbd>" && close.as_ref() == "</kbd>" =>
        {
            Some(i + 3)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn html(markdown: &str) -> String {
        let events = keep_together(pulldown_cmark::Parser::new(markdown).collect());
        let mut html = String::new();
        pulldown_cmark::html::push_html(&mut html, events.into_iter());
        html
    }

    #[test]
    fn a_spaced_em_dash_stays_with_the_word_before_it() {
        assert_eq!(
            html("the kernel — not the service — decides"),
            "<p>the kernel\u{a0}— not the service\u{a0}— decides</p>\n"
        );
        // A source line break just before the dash, too.
        assert_eq!(
            html("the kernel\n— not it"),
            "<p>the kernel\u{a0}— not it</p>\n"
        );
        // Code is left as written.
        assert_eq!(html("`a — b`"), "<p><code>a — b</code></p>\n");
        assert_eq!(
            html("```text\na — b\n```\n"),
            "<pre><code class=\"language-text\">a — b\n</code></pre>\n"
        );
    }

    #[test]
    fn numeric_ranges_stay_whole() {
        // The word holding the range is kept whole, punctuation and all.
        assert_eq!(
            html("ports 0–65535 and (pages 4–9)."),
            "<p>ports <span class=\"keep-together\">0–65535</span> and (pages \
             <span class=\"keep-together\">4–9).</span></p>\n"
        );
        // Only between digits: a word range is ordinary text.
        assert_eq!(html("Monday–Friday"), "<p>Monday–Friday</p>\n");
    }

    #[test]
    fn key_combinations_stay_whole() {
        assert_eq!(
            html("Press <kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>T</kbd> now, or <kbd>Enter</kbd>."),
            "<p>Press <span class=\"keep-together\"><kbd>Ctrl</kbd>+<kbd>Alt</kbd>+<kbd>T</kbd></span> \
             now, or <kbd>Enter</kbd>.</p>\n"
        );
    }
}
