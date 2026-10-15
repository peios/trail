//! TeX mathematics, rendered to MathML while the site builds. Browsers
//! lay MathML out natively, so a page needs no script or font to show an
//! equation, print bundles show it too, and the Markdown mirrors keep the
//! TeX as written.
//!
//! An equation on its own is a ```` ```math ```` fence; one inside a
//! sentence is a code span between dollar signs, `` $`x^2`$ ``. Both are
//! GitHub's spellings. Plain `$x$` is not math here: documentation is full
//! of dollar signs ("$PATH and $HOME") that would pair up into equations.

use pulldown_cmark::{CodeBlockKind, CowStr, Event, Tag, TagEnd};
use pulldown_latex::config::DisplayMode;
use pulldown_latex::{Parser, RenderConfig, Storage, push_mathml};

/// Turn the two math forms into pulldown-cmark's math events, which the
/// renderer then draws: a `math` fence becomes `DisplayMath`, and a code
/// span with a `$` hard against each side becomes `InlineMath`.
pub fn mark_math(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
    let mut out: Vec<Event> = Vec::with_capacity(events.len());
    let mut events = events.into_iter().peekable();
    while let Some(event) = events.next() {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(ref info)))
                if info.split_whitespace().next() == Some("math") =>
            {
                let mut tex = String::new();
                for event in events.by_ref() {
                    match event {
                        Event::End(TagEnd::CodeBlock) => break,
                        Event::Text(text) => tex.push_str(&text),
                        _ => {}
                    }
                }
                out.push(Event::DisplayMath(tex.trim_end().to_string().into()));
            }
            Event::Code(code) => {
                let opens = matches!(out.last(), Some(Event::Text(text)) if text.ends_with('$'));
                let closes = matches!(events.peek(), Some(Event::Text(text)) if text.starts_with('$'));
                if !(opens && closes) {
                    out.push(Event::Code(code));
                    continue;
                }
                if let Some(Event::Text(before)) = out.pop() {
                    let before = &before[..before.len() - 1];
                    if !before.is_empty() {
                        out.push(Event::Text(CowStr::from(before.to_string())));
                    }
                }
                out.push(Event::InlineMath(code));
                if let Some(Event::Text(after)) = events.next()
                    && after.len() > 1
                {
                    out.push(Event::Text(CowStr::from(after[1..].to_string())));
                }
            }
            event => out.push(event),
        }
    }
    out
}

/// An equation as a `<math>` element carrying its TeX as an annotation
/// (for screen readers and copying), or why the TeX could not be read.
pub fn to_mathml(tex: &str, display: bool) -> Result<String, String> {
    let storage = Storage::new();
    let events: Vec<_> = Parser::new(tex, &storage).collect();
    if let Some(Err(error)) = events.iter().find(|event| event.is_err()) {
        return Err(error.to_string());
    }
    let config = RenderConfig {
        display_mode: if display {
            DisplayMode::Block
        } else {
            DisplayMode::Inline
        },
        annotation: Some(tex),
        ..RenderConfig::default()
    };
    let mut mathml = String::new();
    push_mathml(&mut mathml, events.into_iter(), config).map_err(|error| error.to_string())?;
    // pulldown-latex 0.8 writes TeX's `\ ` and `~` as an `&nbsp;` that it
    // then escapes, which shows the entity's name.
    Ok(mathml.replace("<mtext>&amp;nbsp;</mtext>", "<mtext>\u{a0}</mtext>"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equations_become_annotated_mathml() {
        let mathml = to_mathml(r"\frac{1}{2^N}", false).unwrap();
        assert!(mathml.starts_with("<math"), "{mathml}");
        assert!(mathml.contains("<mfrac>"), "{mathml}");
        assert!(mathml.contains(r#"<annotation encoding="application/x-tex">\frac{1}{2^N}</annotation>"#));
        assert!(to_mathml("x", true).unwrap().contains(r#"display="block""#));
        let spaced = to_mathml(r"a\ (\textrm{mod}~4)", false).unwrap();
        assert!(!spaced.contains("&amp;nbsp;"), "{spaced}");
        assert_eq!(spaced.matches("<mtext>\u{a0}</mtext>").count(), 2, "{spaced}");
    }

    fn marked(markdown: &str) -> Vec<Event<'_>> {
        mark_math(pulldown_cmark::Parser::new(markdown).collect())
    }

    #[test]
    fn math_fences_and_dollar_code_spans_are_equations() {
        let events = marked("Where $`N`$ is the factor:\n\n```math\na \\\\\nb\n```\n");
        assert!(events.contains(&Event::Text("Where ".into())));
        assert!(events.contains(&Event::InlineMath("N".into())));
        assert!(events.contains(&Event::Text(" is the factor:".into())));
        assert!(events.contains(&Event::DisplayMath("a \\\\\nb".into())));
        assert!(!events.iter().any(|e| matches!(e, Event::Code(_) | Event::Start(Tag::CodeBlock(_)))));
    }

    #[test]
    fn dollar_signs_in_prose_and_code_stay_text() {
        for markdown in [
            "Set $PATH and $HOME.",
            "The pattern `^[a-z]+$` and `$` itself.",
            "Costs $5 or `x`$ at most.",
            "```text\n$`x`$\n```\n",
        ] {
            let events = marked(markdown);
            assert!(
                !events.iter().any(|e| matches!(e, Event::InlineMath(_) | Event::DisplayMath(_))),
                "{markdown}: {events:?}"
            );
        }
    }

    #[test]
    fn unreadable_tex_is_an_error() {
        assert!(to_mathml(r"\frac{1}", false).is_err());
        assert!(to_mathml(r"\begin{align*} x", true).is_err());
    }
}
