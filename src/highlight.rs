//! Build-time syntax highlighting.
//!
//! Code blocks are highlighted once, at build time, into `<span>`s
//! carrying *class names* rather than inline colors — so a page's code
//! follows the reader's light/dark choice through the same CSS tokens as
//! everything else, instead of freezing one palette into the markup.

use std::fmt::Write as _;
use std::sync::OnceLock;

use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

use crate::markdown::escape_text;

/// Prefixed so the emitted scope atoms ("keyword", "string", "meta")
/// can never collide with the theme's own class names.
const CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "hl-" };

/// The bundled syntax definitions; parsing them is expensive, so one set
/// is built on first use and shared for the whole build.
///
/// syntect's own defaults stop at about forty languages and are missing
/// several a documentation site cannot do without — TOML above all, but
/// also TypeScript, Zig, Nix, Dockerfile and the shell-session forms.
/// `two-face` carries bat's collection instead: the same definitions,
/// several hundred of them. Their licences ship at
/// /assets/LICENSE-Syntaxes.txt, as those licences require.
fn syntaxes() -> &'static SyntaxSet {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAXES.get_or_init(two_face::syntax::extra_newlines)
}

/// The licences of the bundled syntax definitions, as one text file.
/// Only the ones whose terms require acknowledgement are listed; the
/// rest (Sublime's own permissive licence and friends) do not ask for
/// it. Shipped at /assets/LICENSE-Syntaxes.txt, beside the fonts'.
pub fn licenses() -> String {
    let mut out = String::new();
    for line in [
        "Syntax highlighting in this site's code blocks uses syntax definitions",
        "bundled with Trail, collected by the `two-face` crate from bat's assets.",
        "The licences below are those whose terms require acknowledgement when",
        "the definitions are redistributed.",
        "",
        "The full listing, including the licences that do not require",
        "acknowledgement, is at:",
        two_face::acknowledgement::url(),
    ] {
        out.push_str(line);
        out.push('\n');
    }
    for license in two_face::acknowledgement::listing().for_syntaxes() {
        let _ = write!(
            out,
            "\n\n---- {} ----\n\n{}\n",
            license.rel_path.display(),
            license.text.trim_end()
        );
    }
    out
}

/// Render one code block: the highlighted source inside a wrapper that
/// carries the copy button. An unknown or absent language renders as
/// plain escaped text — no highlighting, same chrome.
pub fn code_block(language: Option<&str>, source: &str) -> String {
    let syntax = language
        .map(str::trim)
        .filter(|language| !language.is_empty())
        .and_then(|language| syntaxes().find_syntax_by_token(language));
    let body = match syntax {
        Some(syntax) => highlight(syntax, source).unwrap_or_else(|| escape_text(source)),
        None => escape_text(source),
    };
    let language_attribute = match language.map(str::trim).filter(|l| !l.is_empty()) {
        Some(language) => format!(" data-language=\"{}\"", escape_text(language)),
        None => String::new(),
    };
    format!(
        "<div class=\"code-block\"{language_attribute}>\
         <button class=\"copy-code\" type=\"button\" data-copy-code>\
         <span class=\"copy-label\">Copy</span></button>\
         <pre><code>{body}</code></pre></div>\n"
    )
}

/// Highlight source into classed spans. None if the highlighter trips —
/// a syntax definition misbehaving should cost the colors, not the build.
fn highlight(syntax: &SyntaxReference, source: &str) -> Option<String> {
    let mut generator = ClassedHTMLGenerator::new_with_class_style(syntax, syntaxes(), CLASS_STYLE);
    for line in LinesWithEndings::from(source) {
        generator
            .parse_html_for_line_which_includes_newline(line)
            .ok()?;
    }
    Some(generator.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_languages_are_highlighted_with_classed_spans() {
        let html = code_block(Some("rust"), "// hi\nlet s = \"x\";\n");
        assert!(html.contains("<div class=\"code-block\" data-language=\"rust\">"));
        assert!(html.contains("data-copy-code"));
        // Classes, never inline colors: the page's theme picks those.
        assert!(html.contains("hl-comment"));
        assert!(html.contains("hl-keyword"));
        assert!(html.contains("hl-string"));
        assert!(!html.contains("style=\"color"));
    }

    #[test]
    fn unknown_and_absent_languages_still_render_escaped_code() {
        for language in [Some("nosuchlang"), None] {
            let html = code_block(language, "a < b && c > d\n");
            assert!(html.contains("a &lt; b &amp;&amp; c &gt; d"));
            assert!(html.contains("<div class=\"code-block\""));
            assert!(!html.contains("hl-keyword"));
        }
        // An unknown language is still announced, for the label and CSS.
        assert!(code_block(Some("nosuchlang"), "x\n").contains("data-language=\"nosuchlang\""));
    }

    #[test]
    fn the_extended_set_covers_the_languages_the_defaults_miss() {
        // syntect's own defaults stop short of these; the docs promise
        // them, so a change of syntax set has to fail here first.
        for (language, source) in [
            ("toml", "# hi\nkey = \"value\"\n"),
            ("typescript", "// hi\nconst s: string = \"x\";\n"),
            ("dockerfile", "# hi\nFROM alpine\n"),
            ("nix", "# hi\n{ pkgs }: pkgs.hello\n"),
            ("zig", "// hi\nconst x = 1;\n"),
        ] {
            let html = code_block(Some(language), source);
            assert!(
                html.contains("hl-comment"),
                "{language} was not highlighted: {html}"
            );
        }
    }

    #[test]
    fn syntax_licenses_are_shippable() {
        let text = licenses();
        assert!(text.contains("two-face"));
        assert!(text.len() > 500, "acknowledgements look empty");
    }

    #[test]
    fn aliases_resolve_to_their_syntax() {
        // "sh" and "rs" are aliases, not syntax names.
        assert!(code_block(Some("sh"), "echo hi\n").contains("hl-"));
        assert!(code_block(Some("rs"), "let x = 1;\n").contains("hl-keyword"));
    }
}
