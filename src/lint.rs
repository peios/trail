//! `trail lint`: report breaches of a site's writing rules, each with its
//! file, line and column.
//!
//! Every rule is off until the root trail.toml's `[lint]` table lists it,
//! and every word list is the site's own, so Trail itself holds no house
//! style. The lint is a separate command, never part of a build: a site
//! that doesn't run it is unaffected.
//!
//! It reads sources as written, without running plugins. Prose is what
//! the Markdown parser reports as text, so code spans, code blocks, raw
//! HTML, link destinations and URLs are never checked as words.
//! Frontmatter and trail.toml `title` and `description` values are prose
//! too; their keys are not.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use pulldown_cmark::{CodeBlockKind, Event, LinkType, Parser, Tag, TagEnd};
use serde::Deserialize;

use crate::titlecase;

/// The rules Trail implements. Word-list rules are named by the site.
const BUILTIN_RULES: &[&str] = &[
    "em-dash",
    "double-hyphen",
    "curly-quotes",
    "invisible-characters",
    "exclamation",
    "ampersand",
    "double-space",
    "title-case",
    "heading-case",
    "heading-punctuation",
    "heading-levels",
    "code-language",
    "shell-prompt",
    "line-length",
    "hex-case",
    "serial-comma",
];

/// Rules that report warnings unless the site says otherwise: heuristics
/// that are right often enough to show, not often enough to fail on.
const DEFAULT_WARNINGS: &[&str] = &["serial-comma"];

/// Languages whose blocks must not show a `$ ` prompt.
const SHELL_LANGUAGES: &[&str] = &["sh", "bash", "shell", "zsh"];

/// Stands for a code span inside prose: not a word, so word rules never
/// match across it, but a token, so it can be an item in a list.
const CODE_PLACEHOLDER: char = '\u{E000}';

/// The root trail.toml's `[lint]` table.
#[derive(Debug, Default, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct LintConfig {
    /// The rules that run: built-in rule names, and the `rule` names of
    /// `words` entries.
    #[serde(default)]
    pub rules: Vec<String>,
    /// Rules that report warnings rather than errors. `serial-comma`
    /// always does.
    #[serde(default)]
    pub warnings: Vec<String>,
    /// The longest a prose line may be, in characters.
    #[serde(default = "default_line_length")]
    pub line_length: usize,
    /// Names whose casing is fixed, exempt from the title and heading
    /// case rules: "Peios", "Registry Editor", "netd".
    #[serde(default)]
    pub proper_nouns: Vec<String>,
    #[serde(default)]
    pub words: Vec<WordRule>,
    #[serde(default)]
    pub ignore: Vec<IgnoreRule>,
}

fn default_line_length() -> usize {
    78
}

/// Words or phrases prose must not use.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct WordRule {
    /// What to find, as whole words. A trailing `*` matches any ending:
    /// `colour*` finds "colours" and "coloured".
    #[serde(rename = "match")]
    pub matches: Vec<String>,
    /// The rule these words belong to, for turning them on and off and
    /// for suppressing them. Default `words`.
    #[serde(default = "default_word_rule")]
    pub rule: String,
    /// What to write instead.
    #[serde(default, rename = "use")]
    pub replacement: Option<String>,
    /// Why, if `use` doesn't say it.
    #[serde(default)]
    pub message: Option<String>,
    /// Match case exactly: "may" but not the keyword "MAY".
    #[serde(default)]
    pub case_sensitive: bool,
}

fn default_word_rule() -> String {
    "words".to_string()
}

/// Rules that don't apply under a path: a book whose own conventions
/// differ, or a page that quotes what the rules forbid.
#[derive(Debug, Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct IgnoreRule {
    /// A file or directory, relative to the site root.
    pub path: String,
    /// The rules to skip there; every rule if empty.
    #[serde(default)]
    pub rules: Vec<String>,
}

impl LintConfig {
    /// Reject rule names that name nothing, so a typo can't switch a rule
    /// off silently.
    pub fn validate(&self) -> Result<()> {
        let known = self.known_rules();
        let check = |name: &str, place: &str| -> Result<()> {
            if known.contains(name) {
                Ok(())
            } else {
                bail!(
                    "[lint] {place} names unknown rule '{name}' (rules: {})",
                    known.iter().cloned().collect::<Vec<_>>().join(", ")
                )
            }
        };
        for rule in &self.rules {
            check(rule, "rules")?;
        }
        for rule in &self.warnings {
            check(rule, "warnings")?;
        }
        for ignore in &self.ignore {
            for rule in &ignore.rules {
                check(rule, "ignore")?;
            }
        }
        for words in &self.words {
            if BUILTIN_RULES.contains(&words.rule.as_str()) {
                bail!(
                    "[[lint.words]] rule '{}' is a built-in rule's name",
                    words.rule
                );
            }
            if words.matches.is_empty() || words.matches.iter().any(|m| m.trim().is_empty()) {
                bail!("[[lint.words]] rule '{}' has an empty match", words.rule);
            }
        }
        Ok(())
    }

    fn known_rules(&self) -> BTreeSet<String> {
        BUILTIN_RULES
            .iter()
            .map(|rule| rule.to_string())
            .chain(self.words.iter().map(|words| words.rule.clone()))
            .collect()
    }

    fn on(&self, rule: &str) -> bool {
        self.rules.iter().any(|r| r == rule)
    }

    fn is_warning(&self, rule: &str) -> bool {
        DEFAULT_WARNINGS.contains(&rule) || self.warnings.iter().any(|r| r == rule)
    }
}

/// One breach, before it is placed in a file.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub line: usize,
    pub column: usize,
    pub rule: String,
    pub message: String,
}

/// Lint the site at `root`, print what was found, and say whether any of
/// it is an error.
pub fn run(root: &Path) -> Result<bool> {
    let config_path = root.join("trail.toml");
    let text = fs::read_to_string(&config_path)
        .with_context(|| format!("reading {}", config_path.display()))?;
    let table: toml::Table =
        toml::from_str(&text).with_context(|| format!("parsing {}", config_path.display()))?;
    let Some(lint) = table.get("lint") else {
        bail!(
            "{} has no [lint] table: no rules to check",
            config_path.display()
        );
    };
    let config: LintConfig = lint.clone().try_into().context("reading [lint]")?;
    config.validate()?;

    let attachments: Vec<crate::attachments::AttachmentConfig> = table
        .get("attachments")
        .map(|value| value.clone().try_into())
        .transpose()
        .context("reading [[attachments]]")?
        .unwrap_or_default();
    let mut excluded = Vec::new();
    for attachment in attachments {
        attachment.validate(root, &root.join("dist"))?;
        excluded.push(root.join(attachment.source));
    }
    let mut files = Vec::new();
    collect_files(root, root, &excluded, &mut files)?;
    let mut reports = Vec::new();
    for path in files {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let source =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let findings = if relative.ends_with(".md") {
            lint_markdown(&source, &config)
        } else {
            lint_toml(&source, &config)
        };
        for finding in findings {
            if !ignored(&config, &relative, &finding.rule) {
                reports.push((relative.clone(), finding));
            }
        }
    }
    reports.sort_by(|(a, x), (b, y)| (a, x.line, x.column).cmp(&(b, y.line, y.column)));

    let (mut errors, mut warnings) = (0, 0);
    for (path, finding) in &reports {
        let severity = if config.is_warning(&finding.rule) {
            warnings += 1;
            "warning: "
        } else {
            errors += 1;
            ""
        };
        eprintln!(
            "{}: {severity}{path}:{}:{}: {} [{}]",
            crate::diagnostic::program(),
            finding.line,
            finding.column,
            crate::diagnostic::single_line(&finding.message),
            finding.rule
        );
    }
    let plural = |n: usize, word: &str| format!("{n} {word}{}", if n == 1 { "" } else { "s" });
    eprintln!(
        "{}: {}, {}",
        crate::diagnostic::program(),
        plural(errors, "error"),
        plural(warnings, "warning")
    );
    Ok(errors > 0)
}

/// Every article, trail.toml below the root, and link file. The output
/// directory and dot entries are skipped.
fn collect_files(
    root: &Path,
    dir: &Path,
    excluded: &[PathBuf],
    files: &mut Vec<PathBuf>,
) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .and_then(Iterator::collect)
        .with_context(|| format!("reading {}", dir.display()))?;
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        if name.starts_with('.')
            || (dir == root && name == "dist")
            || excluded.iter().any(|source| path.starts_with(source))
        {
            continue;
        }
        if path.is_dir() {
            collect_files(root, &path, excluded, files)?;
        } else if name.ends_with(".md")
            || name.ends_with(".link")
            || (name == "trail.toml" && dir != root)
        {
            files.push(path);
        }
    }
    Ok(())
}

fn ignored(config: &LintConfig, path: &str, rule: &str) -> bool {
    config.ignore.iter().any(|ignore| {
        let prefix = ignore.path.trim_end_matches('/');
        let under = path == prefix || path.starts_with(&format!("{prefix}/"));
        under && (ignore.rules.is_empty() || ignore.rules.iter().any(|r| r == rule))
    })
}

// --------------------------------------------------------------------------
// Prose
// --------------------------------------------------------------------------

/// A stretch of prose: the text a reader sees within one block, with
/// where each piece of it sits in the source.
#[derive(Debug, Default)]
struct Run {
    text: String,
    /// (offset in `text`, offset in the source), one per piece appended.
    map: Vec<(usize, usize)>,
}

impl Run {
    fn push(&mut self, text: &str, source: usize) {
        self.map.push((self.text.len(), source));
        self.text.push_str(text);
    }

    fn source_offset(&self, at: usize) -> usize {
        let index = self
            .map
            .partition_point(|&(text, _)| text <= at)
            .saturating_sub(1);
        let (text, source) = self.map.get(index).copied().unwrap_or((0, 0));
        source + (at - text)
    }

    /// Blank out URLs, which are not words.
    fn mask_urls(&mut self) {
        let mut masked = String::with_capacity(self.text.len());
        let mut rest = self.text.as_str();
        while let Some(start) = ["https://", "http://", "www."]
            .iter()
            .filter_map(|scheme| rest.find(scheme))
            .min()
        {
            let end = rest[start..]
                .find(char::is_whitespace)
                .map_or(rest.len(), |space| start + space);
            masked.push_str(&rest[..start]);
            masked.extend(
                rest[start..end]
                    .chars()
                    .flat_map(|c| std::iter::repeat_n(' ', c.len_utf8())),
            );
            rest = &rest[end..];
        }
        masked.push_str(rest);
        self.text = masked;
    }
}

/// What a body holds, for the rules to look at.
#[derive(Debug, Default)]
struct Body {
    runs: Vec<Run>,
    /// Code spans: their text and where it starts in the source.
    code_spans: Vec<(String, usize)>,
    /// (level, text without code spans, source offset).
    headings: Vec<(u8, String, usize)>,
    code_blocks: Vec<CodeBlock>,
    /// Source byte ranges that are not prose lines: code blocks,
    /// tables, raw HTML.
    not_prose: Vec<Range<usize>>,
    /// Source byte ranges of code blocks.
    code: Vec<Range<usize>>,
}

#[derive(Debug)]
struct CodeBlock {
    /// Whether it is fenced: an indented block can't carry a language.
    fenced: bool,
    /// The info string; empty for a fence without one.
    info: String,
    /// Where the opening fence starts.
    at: usize,
    /// Its content lines, with where each starts.
    lines: Vec<(usize, String)>,
}

/// Read a Markdown body (already split from its frontmatter).
fn read_body(body: &str) -> Body {
    let mut out = Body::default();
    let mut run = Run::default();
    let mut in_code = false;
    let mut url_text = 0usize;
    let mut strong = 0usize;
    let mut heading: Option<(u8, String, usize)> = None;
    let mut block: Option<CodeBlock> = None;

    let flush = |run: &mut Run, runs: &mut Vec<Run>| {
        if !run.text.trim().is_empty() {
            let mut done = std::mem::take(run);
            done.mask_urls();
            runs.push(done);
        } else {
            *run = Run::default();
        }
    };

    for (event, range) in
        Parser::new_ext(body, crate::markdown::parser_options()).into_offset_iter()
    {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                flush(&mut run, &mut out.runs);
                in_code = true;
                out.not_prose.push(range.clone());
                out.code.push(range.clone());
                let (fenced, info) = match kind {
                    CodeBlockKind::Fenced(info) => (true, info.to_string()),
                    CodeBlockKind::Indented => (false, String::new()),
                };
                block = Some(CodeBlock {
                    fenced,
                    info,
                    at: range.start,
                    lines: Vec::new(),
                });
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code = false;
                if let Some(done) = block.take() {
                    out.code_blocks.push(done);
                }
            }
            Event::Text(text) if in_code => {
                if let Some(CodeBlock { lines, .. }) = block.as_mut() {
                    let mut at = range.start;
                    for line in text.split_inclusive('\n') {
                        lines.push((at, line.trim_end_matches('\n').to_string()));
                        at += line.len();
                    }
                }
            }
            Event::Start(Tag::Table(_)) => {
                flush(&mut run, &mut out.runs);
                out.not_prose.push(range.clone());
            }
            Event::Html(_) => {
                flush(&mut run, &mut out.runs);
                out.not_prose.push(range.clone());
            }
            Event::Start(Tag::Link {
                link_type: LinkType::Autolink | LinkType::Email,
                ..
            }) => {
                url_text += 1;
                run.push(" ", range.start);
            }
            Event::End(TagEnd::Link) if url_text > 0 => url_text -= 1,
            Event::Start(Tag::Heading { level, .. }) => {
                flush(&mut run, &mut out.runs);
                heading = Some((level as u8, String::new(), range.start));
            }
            Event::End(TagEnd::Heading(_)) => {
                flush(&mut run, &mut out.runs);
                if let Some(done) = heading.take() {
                    out.headings.push(done);
                }
            }
            Event::Start(Tag::Strong) => strong += 1,
            Event::End(TagEnd::Strong) => strong = strong.saturating_sub(1),
            Event::Text(text) => {
                if url_text > 0 {
                    run.push(&" ".repeat(text.len()), range.start);
                    continue;
                }
                if strong > 0 {
                    // Bold names a UI element exactly as it appears, so its
                    // "&" is part of a name ("**Language & Formats**").
                    run.push(&text.replace('&', "\u{1}"), range.start);
                } else {
                    run.push(&text, range.start);
                }
                if let Some((_, words, _)) = heading.as_mut() {
                    words.push_str(&text);
                }
            }
            Event::Code(code) => {
                run.push(&CODE_PLACEHOLDER.to_string(), range.start);
                let quoted = &body[range.clone()];
                let offset = quoted.find(code.as_ref()).unwrap_or(0);
                out.code_spans
                    .push((code.to_string(), range.start + offset));
                if let Some((_, words, _)) = heading.as_mut() {
                    words.push(CODE_PLACEHOLDER);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                run.push(" ", range.start);
                if let Some((_, words, _)) = heading.as_mut() {
                    words.push(' ');
                }
            }
            Event::InlineHtml(_) => {}
            Event::Start(
                Tag::Emphasis | Tag::Strikethrough | Tag::Link { .. } | Tag::Image { .. },
            )
            | Event::End(TagEnd::Emphasis | TagEnd::Strikethrough | TagEnd::Link | TagEnd::Image) =>
                {}
            // Every other start or end is a block boundary.
            _ => flush(&mut run, &mut out.runs),
        }
    }
    flush(&mut run, &mut out.runs);
    out
}

/// Line numbers (from 1) and columns (from 1, in characters) of byte
/// offsets in a source.
struct Lines {
    starts: Vec<usize>,
}

impl Lines {
    fn new(source: &str) -> Lines {
        let mut starts = vec![0];
        starts.extend(source.match_indices('\n').map(|(at, _)| at + 1));
        Lines { starts }
    }

    fn locate(&self, source: &str, offset: usize) -> (usize, usize) {
        let line = self.starts.partition_point(|&start| start <= offset);
        let start = self.starts[line - 1];
        let offset = offset.min(source.len());
        let column = source
            .get(start..offset)
            .map_or(1, |text| text.chars().count() + 1);
        (line, column)
    }
}

// --------------------------------------------------------------------------
// Files
// --------------------------------------------------------------------------

/// Lint a Markdown article.
pub fn lint_markdown(source: &str, config: &LintConfig) -> Vec<Finding> {
    let lines = Lines::new(source);
    let mut findings = Vec::new();
    let mut add = |offset: usize, rule: &str, message: String| {
        let (line, column) = lines.locate(source, offset);
        findings.push(Finding {
            line,
            column,
            rule: rule.to_string(),
            message,
        });
    };

    // Frontmatter: its title and description are prose.
    let (frontmatter, body_start) = match source
        .strip_prefix("---\n")
        .and_then(|rest| rest.find("\n---\n").map(|end| (end, rest)))
    {
        Some((end, rest)) => (Some(&rest[..end]), 4 + end + 5),
        None => (None, 0),
    };
    let mut runs = Vec::new();
    if let Some(frontmatter) = frontmatter {
        runs.extend(frontmatter_runs(frontmatter, 4));
        if config.on("title-case")
            && let Ok(yaml) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(frontmatter)
            && let Some(title) = yaml.get("title").and_then(|t| t.as_str())
            && let Some(message) = titlecase::check(title, &config.proper_nouns)
        {
            let at = frontmatter.find("title:").map_or(4, |at| 4 + at);
            add(at, "title-case", message);
        }
    }

    let body_text = &source[body_start.min(source.len())..];
    let body = read_body(body_text);
    for mut run in body.runs {
        for (_, source_at) in &mut run.map {
            *source_at += body_start;
        }
        runs.push(run);
    }
    for run in &runs {
        for (at, rule, message) in prose_findings(&run.text, config) {
            add(run.source_offset(at), &rule, message);
        }
    }

    for (code, at) in &body.code_spans {
        if config.on("hex-case") {
            for (offset, message) in hex_findings(code) {
                add(body_start + at + offset, "hex-case", message);
            }
        }
    }

    // Headings.
    let mut previous = 1u8;
    for (level, text, at) in &body.headings {
        let at = body_start + at;
        if config.on("heading-levels") {
            if *level == 1 {
                add(
                    at,
                    "heading-levels",
                    "use `##` for top-level headings; the title is the page's `#`".into(),
                );
            } else if *level > previous + 1 {
                add(
                    at,
                    "heading-levels",
                    format!("heading skips from level {previous} to level {level}"),
                );
            } else if *level >= 4 {
                add(
                    at,
                    "heading-levels",
                    "avoid `####`: split the page instead".into(),
                );
            }
        }
        previous = *level;
        let visible = text.trim();
        if config.on("heading-punctuation")
            && let Some(last) = visible.chars().last()
            && ".,:;!?".contains(last)
        {
            add(
                at,
                "heading-punctuation",
                format!("heading ends with \"{last}\""),
            );
        }
        if config.on("heading-case")
            && let Some(message) = sentence_case(visible, &config.proper_nouns)
        {
            add(at, "heading-case", message);
        }
    }

    // Code blocks.
    for CodeBlock {
        fenced,
        info,
        at,
        lines: content,
    } in &body.code_blocks
    {
        let at = body_start + at;
        let language = info.split_whitespace().next().unwrap_or("");
        if config.on("code-language") && *fenced && language.is_empty() {
            add(
                at,
                "code-language",
                "tag the code block with its language (`text` for none)".into(),
            );
        }
        if config.on("shell-prompt") && SHELL_LANGUAGES.contains(&language) {
            for (line_at, line) in content {
                if line.trim_start().starts_with("$ ") {
                    add(
                        body_start + line_at,
                        "shell-prompt",
                        "write the command without a `$ ` prompt".into(),
                    );
                }
            }
        }
    }

    // Whole lines.
    let shift = |ranges: &[Range<usize>]| -> Vec<Range<usize>> {
        ranges
            .iter()
            .map(|r| r.start + body_start..r.end + body_start)
            .collect()
    };
    let (not_prose, code) = (shift(&body.not_prose), shift(&body.code));
    for (index, &start) in lines.starts.iter().enumerate() {
        let end = lines
            .starts
            .get(index + 1)
            .map_or(source.len(), |next| next - 1);
        let line = &source[start..end.max(start)];
        let in_code = code.iter().any(|range| range.contains(&start));
        if config.on("invisible-characters") && !in_code {
            for (column, c) in line.char_indices() {
                if INVISIBLE.contains(&c) {
                    add(
                        start + column,
                        "invisible-characters",
                        format!(
                            "{} in source: use a plain space, or nothing",
                            invisible_name(c)
                        ),
                    );
                }
            }
        }
        let prose = start >= body_start && !not_prose.iter().any(|range| range.contains(&start));
        if config.on("line-length") && prose {
            let width = line.chars().count();
            if width > config.line_length && !link_dominated(line, config.line_length) {
                let at = line
                    .char_indices()
                    .nth(config.line_length)
                    .map_or(line.len(), |(at, _)| at);
                add(
                    start + at,
                    "line-length",
                    format!(
                        "line is {width} characters; wrap prose at {}",
                        config.line_length
                    ),
                );
            }
        }
    }

    let suppressions = Suppressions::read(source);
    findings.retain(|finding| !suppressions.covers(finding.line, &finding.rule));
    findings
}

/// The `title` and `description` values of a frontmatter block, a run
/// per key, starting `offset` bytes into the file.
fn frontmatter_runs(frontmatter: &str, offset: usize) -> Vec<Run> {
    let mut runs = Vec::new();
    let mut current: Option<Run> = None;
    let mut at = offset;
    for line in frontmatter.split_inclusive('\n') {
        let content = line.trim_end_matches('\n');
        let key = content
            .split_once(':')
            .filter(|(key, _)| !key.starts_with(' ') && !key.contains(' '));
        if let Some((key, value)) = key {
            if let Some(done) = current.take() {
                runs.push(done);
            }
            if key == "title" || key == "description" {
                let mut run = Run::default();
                let value_at = at + key.len() + 1;
                let trimmed = value.trim_start();
                let lead = value.len() - trimmed.len();
                let unquoted = trimmed.trim_matches(|c| c == '"' || c == '\'');
                let quote = trimmed.len() - trimmed.trim_start_matches(['"', '\'']).len();
                if !unquoted.is_empty() && !matches!(unquoted, ">" | "|" | ">-" | "|-") {
                    run.push(unquoted, value_at + lead + quote);
                }
                current = Some(run);
            }
        } else if let Some(run) = current.as_mut() {
            let trimmed = content.trim_start();
            if !trimmed.is_empty() {
                run.push(" ", at);
                run.push(
                    trimmed.trim_matches(|c| c == '"' || c == '\''),
                    at + (content.len() - trimmed.len()),
                );
            }
        }
        at += line.len();
    }
    runs.extend(current);
    for run in &mut runs {
        run.mask_urls();
    }
    runs
}

/// Lint a trail.toml or link file's `title` and `description`.
pub fn lint_toml(source: &str, config: &LintConfig) -> Vec<Finding> {
    let lines = Lines::new(source);
    let mut findings = Vec::new();
    let Ok(table) = toml::from_str::<toml::Table>(source) else {
        return findings;
    };
    for key in ["title", "description"] {
        let Some(value) = table.get(key).and_then(|v| v.as_str()) else {
            continue;
        };
        let at = source
            .match_indices(key)
            .find(|(at, _)| source[..*at].ends_with('\n') || *at == 0)
            .map_or(0, |(at, _)| at);
        let value_at = source[at..]
            .find(['"', '\''])
            .map_or(at, |quote| at + quote + 1);
        let mut add = |offset: usize, rule: &str, message: String| {
            let (line, column) = lines.locate(source, offset);
            findings.push(Finding {
                line,
                column,
                rule: rule.to_string(),
                message,
            });
        };
        if key == "title"
            && config.on("title-case")
            && let Some(message) = titlecase::check(value, &config.proper_nouns)
        {
            add(at, "title-case", message);
        }
        let mut run = Run::default();
        run.push(value, value_at);
        run.mask_urls();
        for (offset, rule, message) in prose_findings(&run.text, config) {
            add(run.source_offset(offset), &rule, message);
        }
    }
    findings
}

// --------------------------------------------------------------------------
// Rules over prose
// --------------------------------------------------------------------------

/// Every finding in one run of prose: (offset in the run, rule, message).
fn prose_findings(text: &str, config: &LintConfig) -> Vec<(usize, String, String)> {
    let mut out = Vec::new();
    let mut add =
        |at: usize, rule: &str, message: String| out.push((at, rule.to_string(), message));

    for words in &config.words {
        if !config.on(&words.rule) {
            continue;
        }
        for phrase in &words.matches {
            for (at, found) in find_phrase(text, phrase, words.case_sensitive) {
                let message = match (&words.replacement, &words.message) {
                    (Some(replacement), _) => format!("\"{found}\": use \"{replacement}\""),
                    (None, Some(message)) => format!("\"{found}\": {message}"),
                    (None, None) => format!("don't use \"{found}\""),
                };
                add(at, &words.rule, message);
            }
        }
    }

    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let before = |i: usize| if i == 0 { None } else { Some(chars[i - 1].1) };
    let after = |i: usize| chars.get(i + 1).map(|&(_, c)| c);
    for (i, &(at, c)) in chars.iter().enumerate() {
        match c {
            '\u{2014}' if config.on("em-dash") => {
                let spaced = |c: Option<char>| c.is_none_or(char::is_whitespace);
                if !spaced(before(i)) || !spaced(after(i)) {
                    add(
                        at,
                        "em-dash",
                        "put a space on each side of an em dash".into(),
                    );
                }
            }
            '\u{2018}' | '\u{2019}' | '\u{201C}' | '\u{201D}' if config.on("curly-quotes") => {
                add(at, "curly-quotes", format!("\"{c}\": use a straight quote"));
            }
            // As punctuation: after a word and before a space or the end,
            // not an operator such as `!=` or "NOT (!)".
            '!' if config.on("exclamation")
                && before(i).is_some_and(char::is_alphanumeric)
                && after(i).is_none_or(|c| c.is_whitespace() || "\"')]".contains(c)) =>
            {
                add(at, "exclamation", "don't use exclamation points".into());
            }
            // Not the operator `&&`. A bold UI label ("**Language &
            // Formats**") is a literal name, and its "&" never gets here.
            '&' if config.on("ampersand") && before(i) != Some('&') && after(i) != Some('&') => {
                add(at, "ampersand", "write \"and\", not \"&\"".into());
            }
            '.' if config.on("double-space")
                && after(i) == Some(' ')
                && chars.get(i + 2).is_some_and(|&(_, c)| c == ' ')
                && chars.get(i + 3).is_some_and(|&(_, c)| !c.is_whitespace()) =>
            {
                add(at, "double-space", "use one space after a period".into());
            }
            // "--" as a dash: "a -- b" or "a--b". Not "--force", a flag
            // that belongs in code formatting rather than a dash.
            '-' if config.on("double-hyphen")
                && after(i) == Some('-')
                && before(i) != Some('-')
                && chars.get(i + 2).is_none_or(|&(_, c)| c != '-')
                && {
                    let next = chars.get(i + 2).map(|&(_, c)| c);
                    let word_before = before(i).is_some_and(char::is_alphanumeric);
                    next.is_none_or(char::is_whitespace) || word_before
                } =>
            {
                add(
                    at,
                    "double-hyphen",
                    "write an em dash as \"—\", not \"--\"".into(),
                );
            }
            _ => {}
        }
    }

    if config.on("em-dash") {
        for (start, sentence) in sentences(text) {
            let dashes: Vec<usize> = sentence
                .match_indices('\u{2014}')
                .map(|(at, _)| at)
                .collect();
            if dashes.len() > 2 {
                add(
                    start + dashes[2],
                    "em-dash",
                    "more than one pair of em dashes in a sentence".into(),
                );
            }
        }
    }
    if config.on("hex-case") {
        for (at, message) in hex_findings(text) {
            add(at, "hex-case", message);
        }
    }
    if config.on("serial-comma") {
        for at in missing_serial_commas(text) {
            add(
                at,
                "serial-comma",
                "add a serial comma before the last item".into(),
            );
        }
    }
    out
}

/// Whole-word (or whole-phrase) matches of `phrase`: (offset, text found).
fn find_phrase(text: &str, phrase: &str, case_sensitive: bool) -> Vec<(usize, String)> {
    let (phrase, wildcard) = match phrase.strip_suffix('*') {
        Some(stem) => (stem, true),
        None => (phrase, false),
    };
    let parts: Vec<&str> = phrase.split_whitespace().collect();
    let Some(first) = parts.first() else {
        return Vec::new();
    };
    let same = |a: &str, b: &str| {
        if case_sensitive {
            a == b
        } else {
            a.eq_ignore_ascii_case(b)
        }
    };
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut found = Vec::new();
    let mut from = 0;
    while from < text.len() {
        let Some(start) = (from..text.len())
            .filter(|&at| text.is_char_boundary(at))
            .find(|&at| {
                text.get(at..at + first.len())
                    .is_some_and(|candidate| same(candidate, first))
            })
        else {
            break;
        };
        from = start + text[start..].chars().next().map_or(1, char::len_utf8);
        let left_ok = !first.starts_with(is_word)
            || text[..start]
                .chars()
                .next_back()
                .is_none_or(|c| !is_word(c));
        if !left_ok {
            continue;
        }
        let mut end = start + first.len();
        let mut matched = true;
        for part in &parts[1..] {
            let gap = text[end..].len() - text[end..].trim_start().len();
            if gap == 0
                || !text
                    .get(end + gap..end + gap + part.len())
                    .is_some_and(|candidate| same(candidate, part))
            {
                matched = false;
                break;
            }
            end += gap + part.len();
        }
        if !matched {
            continue;
        }
        if wildcard {
            end += text[end..]
                .find(|c: char| !is_word(c))
                .unwrap_or(text.len() - end);
        }
        let last = parts.last().unwrap_or(first);
        let right_ok =
            !last.ends_with(is_word) || text[end..].chars().next().is_none_or(|c| !is_word(c));
        if right_ok {
            found.push((start, text[start..end].to_string()));
        }
    }
    found
}

/// Sentences of a run: (offset, text). A sentence ends at ". ", "? ", "! "
/// or the end of the run.
fn sentences(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start = 0;
    let bytes = text.as_bytes();
    for (at, c) in text.char_indices() {
        if matches!(c, '.' | '?' | '!') && bytes.get(at + 1).is_none_or(|b| b.is_ascii_whitespace())
        {
            out.push((start, &text[start..at + 1]));
            start = at + 1;
        }
    }
    if start < text.len() {
        out.push((start, &text[start..]));
    }
    out
}

/// Lowercase digits in `0x` literals: (offset, message).
fn hex_findings(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (at, _) in text.match_indices("0x") {
        if text[..at]
            .chars()
            .next_back()
            .is_some_and(char::is_alphanumeric)
        {
            continue;
        }
        let digits: String = text[at + 2..]
            .chars()
            .take_while(char::is_ascii_hexdigit)
            .collect();
        if digits.chars().any(|c| c.is_ascii_lowercase()) {
            out.push((
                at,
                format!(
                    "write hexadecimal digits in uppercase: 0x{}",
                    digits.to_ascii_uppercase()
                ),
            ));
        }
    }
    out
}

/// Where a list of three or more items lacks the comma before its final
/// "and" or "or": "users, groups and services". A heuristic: each item
/// must be short (at most three words), so that "When it starts, it runs
/// and stops" (two clauses, not a list) passes.
fn missing_serial_commas(text: &str) -> Vec<usize> {
    let mut out = Vec::new();
    for (start, sentence) in sentences(text) {
        // Clauses split at punctuation stronger than a comma.
        let mut clause_start = 0;
        for clause in sentence.split_inclusive([';', ':', '(', ')', '—']) {
            check_clause(clause, start + clause_start, &mut out);
            clause_start += clause.len();
        }
    }
    out
}

/// Report "A, B and C" in one clause. The pieces before the conjunction
/// are split at commas; it is a list only if each looks like a list item.
fn check_clause(clause: &str, offset: usize, out: &mut Vec<usize>) {
    for conjunction in [" and ", " or "] {
        for (at, _) in clause.match_indices(conjunction) {
            let before = &clause[..at];
            // `"is," "carries," or "produces,"`: the comma is in the quote.
            if before
                .trim_end_matches(['"', '\u{201D}', '\''])
                .ends_with(',')
            {
                continue;
            }
            let items: Vec<&str> = before.split(',').collect();
            if items.len() < 2 {
                continue;
            }
            // A comma inside a quotation doesn't separate list items.
            let inner = &before[items[0].len()..];
            if inner.matches('"').count() % 2 == 1 {
                continue;
            }
            // "file names and paths, registry paths, …": more items follow,
            // so this conjunction joins words within one item.
            let after = &clause[at + conjunction.len()..];
            if after.contains(',') {
                continue;
            }
            let short = |item: &str| (1..=3).contains(&item.split_whitespace().count());
            let first = items[0].trim();
            let first_words = first.split_whitespace().count();
            let lone_adverb = first_words == 1
                && (first.to_ascii_lowercase().ends_with("ly")
                    || INTRODUCERS.contains(&first.to_ascii_lowercase().as_str()));
            if first.is_empty()
                || first_words > 5
                || lone_adverb
                || starts_with_any(first, SUBORDINATORS)
                || starts_with_any(first, PREPOSITIONS)
                || !items[1..].iter().all(|item| short(item))
                || items[1..].iter().any(|item| {
                    starts_with_any(item, NOT_ITEMS) || starts_with_any(item, PREPOSITIONS)
                })
                || (starts_with_any(after, NOT_ITEMS) && !after.trim_start().starts_with("so on"))
            {
                continue;
            }
            out.push(offset + at);
        }
    }
}

/// Words that open a subordinate clause, so the piece they start
/// introduces the sentence: "If the key exists, read it and write it".
const SUBORDINATORS: &[&str] = &[
    "if", "when", "whenever", "while", "because", "although", "though", "unless", "once", "since",
    "after", "before", "until", "as", "where", "wherever", "whether",
];

/// Prepositions, whose phrases introduce or qualify rather than list:
/// "For example, tokens and keys"; "a name, in dot- or hyphen-separated
/// segments".
const PREPOSITIONS: &[&str] = &[
    "about",
    "across",
    "against",
    "along",
    "among",
    "at",
    "behind",
    "below",
    "beside",
    "between",
    "beyond",
    "by",
    "despite",
    "during",
    "except",
    "for",
    "from",
    "in",
    "inside",
    "into",
    "of",
    "on",
    "onto",
    "outside",
    "over",
    "per",
    "through",
    "throughout",
    "to",
    "toward",
    "under",
    "unlike",
    "upon",
    "via",
    "with",
    "within",
    "without",
];

/// One-word openings that introduce a sentence: "Together, the
/// specifications and the manuals …".
const INTRODUCERS: &[&str] = &[
    "together",
    "however",
    "otherwise",
    "instead",
    "also",
    "then",
    "first",
    "finally",
    "therefore",
    "thus",
    "here",
    "now",
    "so",
    "still",
    "yet",
    "besides",
    "meanwhile",
    "again",
    "today",
    "later",
];

/// Openings of a piece that is not a list item: a new clause ("it
/// runs"), a contrast ("not `JOB_ID`"), an example ("such as Rust"), a
/// continuation ("and the privileges").
const NOT_ITEMS: &[&str] = &[
    "it",
    "its",
    "they",
    "you",
    "this",
    "that",
    "which",
    "who",
    "whom",
    "whose",
    "what",
    "why",
    "how",
    "he",
    "she",
    "we",
    "there",
    "these",
    "those",
    "if",
    "when",
    "where",
    "while",
    "because",
    "so",
    "but",
    "then",
    "unless",
    "although",
    "though",
    "not",
    "such",
    "including",
    "like",
    "especially",
    "namely",
    "either",
    "neither",
    "both",
    "and",
    "or",
    "nor",
];

fn starts_with_any(text: &str, words: &[&str]) -> bool {
    text.split_whitespace()
        .next()
        .map(|word| {
            word.trim_matches(|c: char| !c.is_alphanumeric())
                .to_ascii_lowercase()
        })
        .is_some_and(|word| words.contains(&word.as_str()))
}

// --------------------------------------------------------------------------
// Headings and lines
// --------------------------------------------------------------------------

/// Whether `heading` breaks sentence case: a capital after the first word
/// that isn't a proper noun, an acronym, or an identifier. Words after a
/// colon may be capitalized.
fn sentence_case(heading: &str, proper_nouns: &[String]) -> Option<String> {
    let words = titlecase::words(heading);
    let fixed = titlecase::fixed_words(&words, proper_nouns);
    let mut after_colon = false;
    for (i, word) in words.iter().enumerate() {
        let colon = after_colon;
        after_colon = word.raw.ends_with(':');
        if fixed.contains(&i) || titlecase::is_identifier(&word.bare) {
            continue;
        }
        let Some(first) = word.bare.chars().next() else {
            continue;
        };
        if i == 0 {
            if first.is_lowercase() {
                return Some(format!(
                    "capitalize \"{}\", or add it to proper_nouns",
                    word.bare
                ));
            }
        } else if first.is_uppercase() && !colon {
            return Some(format!(
                "headings use sentence case: lowercase \"{}\", or add it to proper_nouns",
                word.bare
            ));
        }
    }
    None
}

/// Whether a line's overflow comes from link destinations and URLs: with
/// them removed, it fits.
fn link_dominated(line: &str, limit: usize) -> bool {
    let trimmed = line.trim_start();
    if trimmed.starts_with('[') && trimmed.contains("]: ") {
        return true; // a link reference definition
    }
    let mut kept = String::new();
    let mut rest = line;
    loop {
        let next = [
            rest.find("]("),
            rest.find("http://"),
            rest.find("https://"),
            rest.find('<'),
        ]
        .into_iter()
        .flatten()
        .min();
        let Some(at) = next else {
            kept.push_str(rest);
            break;
        };
        kept.push_str(&rest[..at]);
        let tail = &rest[at..];
        let skip = if let Some(dest) = tail.strip_prefix("](") {
            dest.find(')').map_or(tail.len(), |end| end + 3)
        } else if tail.starts_with('<') {
            tail.find('>').map_or(1, |end| end + 1)
        } else {
            tail.find(char::is_whitespace).unwrap_or(tail.len())
        };
        if tail.starts_with('<') && !tail[1..].starts_with("http") {
            kept.push('<');
            rest = &tail[1..];
            continue;
        }
        rest = &tail[skip..];
    }
    kept.chars().count() <= limit && kept.len() < line.len()
}

const INVISIBLE: &[char] = &[
    '\u{00A0}', '\u{2007}', '\u{202F}', '\u{200B}', '\u{200C}', '\u{200D}', '\u{2060}', '\u{FEFF}',
];

fn invisible_name(c: char) -> &'static str {
    match c {
        '\u{00A0}' => "a no-break space",
        '\u{2007}' => "a figure space",
        '\u{202F}' => "a narrow no-break space",
        '\u{2060}' => "a word joiner",
        '\u{FEFF}' => "a zero-width no-break space",
        _ => "a zero-width character",
    }
}

// --------------------------------------------------------------------------
// Suppression comments
// --------------------------------------------------------------------------

/// Rules switched off by comments in a Markdown source:
///
/// - `<!-- lint-disable-next-line rule … -->` for the line after it,
/// - `<!-- lint-disable rule … -->` until `<!-- lint-enable rule … -->`.
///
/// With no rules named, every rule.
struct Suppressions {
    /// Line (from 1) → rules off there; an empty set means every rule.
    lines: BTreeMap<usize, Option<BTreeSet<String>>>,
}

impl Suppressions {
    fn read(source: &str) -> Suppressions {
        let mut lines = BTreeMap::new();
        let mut active: Option<Option<BTreeSet<String>>> = None;
        let mut in_fence = false;
        for (index, line) in source.lines().enumerate() {
            let number = index + 1;
            let trimmed = line.trim();
            if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
                in_fence = !in_fence;
            }
            if let Some(off) = &active {
                lines.insert(number, off.clone());
            }
            if in_fence {
                continue;
            }
            // A comment may sit in a blockquote ("> <!-- … -->") or a
            // list item (indented).
            let unquoted = trimmed.trim_start_matches(|c: char| c == '>' || c.is_whitespace());
            let Some(directive) = unquoted
                .strip_prefix("<!--")
                .and_then(|d| d.strip_suffix("-->"))
            else {
                continue;
            };
            let mut words = directive.split_whitespace();
            let rules: BTreeSet<String> = words.clone().skip(1).map(str::to_string).collect();
            let rules = (!rules.is_empty()).then_some(rules);
            match words.next() {
                Some("lint-disable-next-line") => {
                    lines.insert(number + 1, rules);
                }
                Some("lint-disable") => active = Some(rules),
                Some("lint-enable") => active = None,
                _ => {}
            }
        }
        Suppressions { lines }
    }

    fn covers(&self, line: usize, rule: &str) -> bool {
        match self.lines.get(&line) {
            Some(None) => true,
            Some(Some(rules)) => rules.contains(rule),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_attachments_are_data_not_articles() {
        let dir = tempfile::tempdir().unwrap();
        crate::attachments::tests::fixture(dir.path());
        fs::write(
            dir.path().join("bundles/tools/README.md"),
            "# Standalone title\n",
        )
        .unwrap();
        let config = dir.path().join("trail.toml");
        fs::write(
            &config,
            fs::read_to_string(&config).unwrap() + "\n[lint]\nrules=['heading-levels']\n",
        )
        .unwrap();
        assert!(!run(dir.path()).unwrap());
        fs::write(
            dir.path()
                .join("alpha.product/5--loose.topic/999--invalid.md"),
            "---\ntitle: Invalid\ntype: concept\n---\n# Invalid heading\n",
        )
        .unwrap();
        assert!(run(dir.path()).unwrap());
    }

    fn config(rules: &[&str]) -> LintConfig {
        let mut config: LintConfig = toml::from_str(
            r#"
            proper_nouns = ["Peios", "Registry Editor", "netd", "regman"]
            [[words]]
            match = ["simply", "of course", "e.g.", "and/or"]
            [[words]]
            match = ["may", "us"]
            case_sensitive = true
            message = "write \"can\" or \"might\""
            [[words]]
            rule = "spelling"
            match = ["colour*", "behaviour"]
            use = "the US spelling"
            "#,
        )
        .unwrap();
        config.rules = rules.iter().map(|rule| rule.to_string()).collect();
        config.validate().unwrap();
        config
    }

    fn page(body: &str) -> String {
        format!("---\ntitle: A Page\ndescription: About things.\n---\n\n{body}")
    }

    /// (line, rule) of each finding.
    fn found(body: &str, rules: &[&str]) -> Vec<(usize, String)> {
        lint_markdown(&page(body), &config(rules))
            .into_iter()
            .map(|finding| (finding.line, finding.rule))
            .collect()
    }

    #[test]
    fn words_match_whole_words_outside_code_and_urls() {
        let body = "Simply run it, e.g. now.\nThe MAY keyword and may.\n\
                    Not mayhem, `simply`, or https://simply.example/may.\n\n```text\nsimply\n```\n";
        assert_eq!(
            found(body, &["words"]),
            vec![
                (6, "words".into()),
                (6, "words".into()),
                (7, "words".into())
            ]
        );
        // Phrases match across a line break; wildcards take any ending.
        assert_eq!(
            found("Of\ncourse, the colours.", &["words", "spelling"]).len(),
            2
        );
        // "US" is not "us" when the match is case-sensitive.
        assert!(found("US English.", &["words"]).is_empty());
    }

    #[test]
    fn word_rules_are_off_until_listed() {
        assert!(found("Simply the colour.", &[]).is_empty());
        assert_eq!(
            found("Simply the colour.", &["spelling"]),
            vec![(6, "spelling".into())]
        );
    }

    #[test]
    fn frontmatter_values_are_prose_and_titles_are_cased() {
        let source =
            "---\ntitle: Developing For Peios\ndescription: Simply\n  everything.\n---\n\nBody.\n";
        let findings = lint_markdown(source, &config(&["words", "title-case"]));
        let rules: Vec<(usize, &str)> =
            findings.iter().map(|f| (f.line, f.rule.as_str())).collect();
        assert_eq!(rules, vec![(2, "title-case"), (3, "words")]);
        assert!(findings[0].message.contains("\"Developing for Peios\""));
    }

    #[test]
    fn em_dashes_need_spaces_and_come_in_one_pair() {
        assert!(found("The kernel — not it — decides.", &["em-dash"]).is_empty());
        assert_eq!(found("The kernel—not it.", &["em-dash"]).len(), 1);
        assert_eq!(
            found("A — b — c — d.", &["em-dash"]),
            vec![(6, "em-dash".into())]
        );
        assert_eq!(found("A -- b.", &["double-hyphen"]).len(), 1);
        assert_eq!(found("A--b.", &["double-hyphen"]).len(), 1);
        assert!(found("Pass `--force`.", &["double-hyphen"]).is_empty());
        // A flag outside code formatting is not a dash.
        assert!(found("Pass --force to it.", &["double-hyphen"]).is_empty());
    }

    #[test]
    fn typography() {
        let rules = [
            "curly-quotes",
            "invisible-characters",
            "exclamation",
            "ampersand",
            "double-space",
        ];
        assert_eq!(found("It\u{2019}s.", &rules).len(), 1);
        assert_eq!(found("A\u{a0}b.", &rules).len(), 1);
        assert_eq!(found("Wow!", &rules).len(), 1);
        assert_eq!(found("A & b.", &rules).len(), 1);
        assert_eq!(found("One.  Two.", &rules).len(), 1);
        assert!(found("One. Two `a!=b && c`.", &rules).is_empty());
        // Operators, and "&" in a bold UI label, are not punctuation.
        assert!(
            found(
                "| NOT (!) | a != b | AND (&&) |\n|---|---|---|\n| x | y | z |\n",
                &rules
            )
            .is_empty()
        );
        assert!(found("Open **Language & Formats**.", &rules).is_empty());
        // Invisible characters in code blocks are the code's business.
        assert!(found("```text\na\u{a0}b\n```\n", &rules).is_empty());
    }

    #[test]
    fn headings() {
        let rules = ["heading-case", "heading-punctuation", "heading-levels"];
        assert!(
            found(
                "## Choosing a registry layer\n\n### The netd book\n",
                &rules
            )
            .is_empty()
        );
        assert!(found("## Using the Registry Editor with SIDs and `Foo`\n", &rules).is_empty());
        assert!(found("## netd and regman\n\n## Notes: Why it matters\n", &rules).is_empty());
        assert_eq!(
            found("## Choosing a Registry Layer\n", &rules),
            vec![(6, "heading-case".into())]
        );
        assert_eq!(
            found("## Why?\n", &rules),
            vec![(6, "heading-punctuation".into())]
        );
        assert_eq!(
            found("# Title\n", &rules),
            vec![(6, "heading-levels".into())]
        );
        assert_eq!(
            found(
                "## One\n\n#### Four\n\n## Two\n\n### Three\n\n#### Four\n",
                &rules
            ),
            vec![(8, "heading-levels".into()), (14, "heading-levels".into())]
        );
    }

    #[test]
    fn code_blocks() {
        let rules = ["code-language", "shell-prompt"];
        assert_eq!(
            found("```\nx\n```\n", &rules),
            vec![(6, "code-language".into())]
        );
        assert_eq!(
            found("```sh\nls\n$ svctl restart netd\n```\n", &rules),
            vec![(8, "shell-prompt".into())]
        );
        assert!(found("```text\n$ shown output\n```\n", &rules).is_empty());
    }

    #[test]
    fn long_prose_lines() {
        let long = "word ".repeat(17);
        assert_eq!(
            found(long.trim(), &["line-length"]),
            vec![(6, "line-length".into())]
        );
        // Tables, code and lines long only because of a link are exempt.
        let link = format!(
            "See [the page](~a/very/long/reference/to/some/page/that/goes/on/and/on#anchor) {}",
            "x"
        );
        let table = format!("| {} |\n|---|\n| x |\n", long.trim());
        let code = format!("```text\n{}\n```\n", long.trim());
        for body in [link, table, code] {
            assert!(found(&body, &["line-length"]).is_empty(), "{body}");
        }
    }

    #[test]
    fn hex_digits_are_uppercase_in_prose_and_code_spans() {
        assert_eq!(
            found("Values 0x1f and `0xc0000022`, but 0x1F.", &["hex-case"]).len(),
            2
        );
        assert!(found("```c\nint x = 0xdeadbeef;\n```\n", &["hex-case"]).is_empty());
    }

    #[test]
    fn serial_commas() {
        for missing in [
            "Users, groups and services.",
            "Read the key, the value or the layer.",
            "It covers `a`, `b` and `c`.",
            "The macros (`.B`, `.I`, `.BR` and so on) apply.",
        ] {
            assert_eq!(found(missing, &["serial-comma"]).len(), 1, "{missing}");
        }
        // Each of these was a false positive on the Peios style guide.
        for fine in [
            "Users, groups, and services.",
            "Words such as \"is,\" \"carries,\" or \"produces,\" state facts.",
            "When it starts, it runs and stops.",
            "If the key exists, read it and write it.",
            "For example, tokens and keys.",
            "Outside that use, write \"can\" or \"might\".",
            "Commands, flags, file names and paths, registry paths, and constants.",
            "`size_installed`, not \"size installed\" or \"`size_installed`s\".",
            "A program, such as sshd or the console sign-in, asks.",
            "Credentials to start a logon, by a person or a client.",
            "The set of logon types, the well-known SIDs, and the privileges and what each one does.",
            "Don't add \"On Peios, …\" notes or a separate section.",
            "Together, the specifications and the manuals are the corpus.",
            "Registry keys and values belong to regman, and events and their fields to evman.",
            "A TRM must not use them, either in uppercase or in lowercase.",
            "What earns an appendix, why an appendix and the body differ.",
        ] {
            assert!(found(fine, &["serial-comma"]).is_empty(), "{fine}");
        }
    }

    #[test]
    fn comments_suppress_lines_and_stretches() {
        let body = "<!-- lint-disable-next-line words -->\nSimply.\nSimply.\n\n\
                    <!-- lint-disable -->\nSimply!\n<!-- lint-enable -->\nSimply.\n\n\
                    > <!-- lint-disable-next-line words -->\n> Simply quoted.\n";
        assert_eq!(
            found(body, &["words", "exclamation"]),
            vec![(8, "words".into()), (13, "words".into())]
        );
    }

    #[test]
    fn paths_suppress_rules_and_unknown_rules_are_errors() {
        let mut config = config(&["hex-case"]);
        config.ignore = toml::from_str::<toml::Table>("path = \"a/book\"\nrules = [\"hex-case\"]")
            .map(|table| vec![table.try_into().unwrap()])
            .unwrap();
        assert!(ignored(&config, "a/book/1--x.md", "hex-case"));
        assert!(!ignored(&config, "a/bookish.md", "hex-case"));
        config.rules.push("no-such-rule".into());
        assert!(
            config
                .validate()
                .unwrap_err()
                .to_string()
                .contains("unknown rule 'no-such-rule'")
        );
    }

    #[test]
    fn toml_titles_are_checked() {
        let findings = lint_toml("title = \"Writing For Peios\"\n", &config(&["title-case"]));
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 1);
    }
}
