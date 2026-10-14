//! Plugin blocks: a fenced block whose info string opens with `{name}`
//! (MyST's spelling of a directive) is handed to an external command the
//! root trail.toml configures under that name, and replaced by the
//! markdown the command returns.
//!
//! Expansion runs once per load, before anything reads an article body,
//! so generated content is indistinguishable from authored content: it
//! defines anchors, its `~` links are checked, it reaches the mirrors and
//! the search index, and it counts toward reading time.
//!
//! Each plugin runs at most once per load and receives every one of its
//! blocks in a single JSON request, so a plugin that cross-references its
//! own output (a function name linking to wherever that function is
//! documented) sees the whole site at once.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Write as _;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};

use anyhow::{Context, Result, bail, ensure};
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};

use crate::site::{Article, Site};

/// The request/response protocol version, sent with every request so a
/// plugin can refuse one it does not understand.
const PROTOCOL: u32 = 1;

/// One `[plugins.<name>]` table in the root trail.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginConfig {
    /// The program and its arguments, run without a shell. A program
    /// containing a `/` is a path relative to the site root; a bare name
    /// is looked up on PATH. The working directory is the site root.
    pub command: Vec<String>,
}

/// A `{name}` fence found in an article body.
#[derive(Debug, PartialEq)]
struct Block {
    name: String,
    /// Everything on the opening line after `{name}`, trimmed.
    argument: String,
    /// The `:key: value` lines that open the block. A flag option's value
    /// is empty.
    options: BTreeMap<String, String>,
    /// What follows the options.
    body: String,
    /// The whole fence within the article body, from the opening fence to
    /// the end of the closing one (its line break excluded).
    range: Range<usize>,
    /// The opening fence's line within the article body, from 1.
    line: usize,
}

/// A problem with one block, reported against its body line.
#[derive(Debug, PartialEq)]
struct BlockError {
    line: usize,
    message: String,
}

#[derive(Serialize)]
struct Request<'a> {
    protocol: u32,
    plugin: &'a str,
    root: String,
    blocks: Vec<RequestBlock<'a>>,
}

#[derive(Serialize)]
struct RequestBlock<'a> {
    id: usize,
    /// The page's site-absolute URL path ("/kernel/driver-api/core").
    page: &'a str,
    /// The page as a `~` reference without the `~` ("kernel/driver-api/core"),
    /// the fully qualified form, which always resolves.
    reference: String,
    /// The article's source file, relative to the site root.
    file: String,
    /// The opening fence's line in `file`, from 1. For messages only.
    line: usize,
    argument: &'a str,
    options: &'a BTreeMap<String, String>,
    body: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    blocks: Vec<ResponseBlock>,
    /// Files the output was derived from, relative to the site root or
    /// absolute. A change to any of them invalidates the cached output,
    /// and `trail serve` rebuilds when one changes.
    #[serde(default)]
    depends_on: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResponseBlock {
    id: usize,
    #[serde(default)]
    markdown: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// A plugin's last successful output, reused while its request and every
/// file it depends on are unchanged. `trail serve` rebuilds on every
/// keystroke's save; without this, each one would rerun every plugin.
struct Cached {
    key: u64,
    dependencies: Vec<(PathBuf, Option<u64>)>,
    outputs: Vec<String>,
}

/// Keyed by (canonical site root, plugin name): one process can load
/// several sites (the test suite does).
fn cache() -> &'static Mutex<HashMap<(PathBuf, String), Cached>> {
    static CACHE: OnceLock<Mutex<HashMap<(PathBuf, String), Cached>>> = OnceLock::new();
    CACHE.get_or_init(Default::default)
}

/// Every plugin's dependencies from the latest load of each site root,
/// for the serve watcher.
fn dependency_registry() -> &'static Mutex<HashMap<PathBuf, Vec<PathBuf>>> {
    static DEPENDENCIES: OnceLock<Mutex<HashMap<PathBuf, Vec<PathBuf>>>> = OnceLock::new();
    DEPENDENCIES.get_or_init(Default::default)
}

/// The files plugins reported depending on in the latest load of the site
/// at `root`, plus each plugin's own program when it is a path.
pub fn dependencies(root: &Path) -> Vec<PathBuf> {
    let root = canonical(root);
    dependency_registry()
        .lock()
        .unwrap()
        .get(&root)
        .cloned()
        .unwrap_or_default()
}

/// Expand every plugin block in the site, in place. Any problem — an
/// unknown plugin, a malformed block, a plugin that fails or returns an
/// error for a block — fails the load, with every problem reported at once.
pub fn expand(site: &mut Site) -> Result<()> {
    let root = site.root.clone();
    let plugins = site.config.plugins.clone();
    let mut programs = BTreeMap::new();
    for (name, config) in &plugins {
        validate_name(name).with_context(|| format!("in [plugins.{name}]"))?;
        programs.insert(name.clone(), program(&root, name, config)?);
    }

    let mut articles = site.articles_mut();
    let mut errors = Vec::new();
    // Per plugin: (article index, block).
    let mut by_plugin: BTreeMap<String, Vec<(usize, Block)>> = BTreeMap::new();
    let mut line_offsets = HashMap::new();
    for (index, article) in articles.iter().enumerate() {
        if !article.body.contains("```") && !article.body.contains("~~~") {
            continue;
        }
        let (blocks, block_errors) = find_blocks(&article.body);
        if blocks.is_empty() && block_errors.is_empty() {
            continue;
        }
        let offset = body_line_offset(article);
        line_offsets.insert(index, offset);
        for error in block_errors {
            errors.push(located(&root, article, offset + error.line, &error.message));
        }
        for block in blocks {
            if plugins.contains_key(&block.name) {
                by_plugin
                    .entry(block.name.clone())
                    .or_default()
                    .push((index, block));
            } else {
                let message = unknown_plugin(&block.name, &plugins);
                errors.push(located(&root, article, offset + block.line, &message));
            }
        }
    }
    if !errors.is_empty() {
        bail!("{}", report(&errors));
    }

    let canonical_root = canonical(&root);
    let mut dependencies = Vec::new();
    let mut replacements: HashMap<usize, Vec<(Range<usize>, String)>> = HashMap::new();
    for (name, blocks) in &by_plugin {
        let config = &plugins[name];
        let program = &programs[name];
        let request_blocks: Vec<RequestBlock> = blocks
            .iter()
            .enumerate()
            .map(|(id, (index, block))| {
                let article = &articles[*index];
                RequestBlock {
                    id,
                    page: &article.path,
                    reference: article.path.trim_start_matches('/').to_string(),
                    file: relative_file(&root, article),
                    line: line_offsets[index] + block.line,
                    argument: &block.argument,
                    options: &block.options,
                    body: &block.body,
                }
            })
            .collect();
        let outcome = run_cached(&canonical_root, name, config, program, request_blocks)
            .with_context(|| format!("running plugin '{name}'"))?;
        dependencies.extend(outcome.dependencies);
        for (id, result) in outcome.outputs.into_iter().enumerate() {
            let (index, block) = &blocks[id];
            match result {
                Ok(markdown) => {
                    let body = &articles[*index].body;
                    let replacement = splice_text(body, &block.range, &markdown);
                    replacements
                        .entry(*index)
                        .or_default()
                        .push((block.range.clone(), replacement));
                }
                Err(message) => errors.push(located(
                    &root,
                    articles[*index],
                    line_offsets[index] + block.line,
                    &format!("{name}: {message}"),
                )),
            }
        }
    }
    dependencies.sort();
    dependencies.dedup();
    dependency_registry()
        .lock()
        .unwrap()
        .insert(canonical_root, dependencies);
    if !errors.is_empty() {
        bail!("{}", report(&errors));
    }

    for (index, mut edits) in replacements {
        let article = &mut articles[index];
        // Back to front, so earlier ranges stay valid.
        edits.sort_by_key(|(range, _)| std::cmp::Reverse(range.start));
        for (range, text) in edits {
            article.body.replace_range(range, &text);
        }
        if !article.authored_minutes {
            article.reading_minutes = crate::site::reading_minutes(&article.body);
        }
    }
    Ok(())
}

/// Find every `{name}` fence in a body. Fences are found by the same
/// parser the renderer uses, so a fence inside a longer fence (a page
/// showing the syntax) or inside an indented code block stays literal,
/// and a fence inside a list item or blockquote is found with its
/// container's prefixes already stripped from its contents.
fn find_blocks(body: &str) -> (Vec<Block>, Vec<BlockError>) {
    let mut blocks = Vec::new();
    let mut errors = Vec::new();
    let mut open: Option<(String, Range<usize>)> = None;
    let mut contents = String::new();
    let parser = Parser::new_ext(body, crate::markdown::parser_options()).into_offset_iter();
    for (event, range) in parser {
        match event {
            Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(info))) => {
                if let Some(rest) = info.trim().strip_prefix('{') {
                    open = Some((rest.to_string(), range));
                    contents.clear();
                }
            }
            Event::Text(text) if open.is_some() => contents.push_str(&text),
            Event::End(TagEnd::CodeBlock) => {
                let Some((info, range)) = open.take() else {
                    continue;
                };
                let line = body[..range.start].matches('\n').count() + 1;
                match parse_block(&info, &contents, body, range, line) {
                    Ok(block) => blocks.push(block),
                    Err(message) => errors.push(BlockError { line, message }),
                }
            }
            _ => {}
        }
    }
    (blocks, errors)
}

/// Parse one fence: `info` is its info string after the opening `{`.
fn parse_block(
    info: &str,
    contents: &str,
    body: &str,
    range: Range<usize>,
    line: usize,
) -> Result<Block, String> {
    let Some((name, argument)) = info.split_once('}') else {
        return Err(format!(
            "plugin fence '{{{info}' has no closing '}}' after its name"
        ));
    };
    validate_name(name).map_err(|error| error.to_string())?;
    if !closes(&body[range.clone()]) {
        return Err(format!("unclosed '{{{name}}}' fence"));
    }
    let (options, rest) = parse_options(contents)?;
    Ok(Block {
        name: name.to_string(),
        argument: argument.trim().to_string(),
        options,
        body: rest,
        range,
        line,
    })
}

/// Whether a fence's source text ends with a closing fence. An unclosed
/// fence runs to the end of its container and would quietly swallow the
/// rest of the page into the plugin's input.
fn closes(source: &str) -> bool {
    let strip = |line: &str| -> String {
        line.trim_start_matches(|c: char| c == '>' || c.is_whitespace())
            .trim_end()
            .to_string()
    };
    let mut lines = source.lines();
    let Some(first) = lines.next().map(strip) else {
        return false;
    };
    let marker = first.chars().next().unwrap_or('`');
    let length = first.chars().take_while(|&c| c == marker).count();
    lines
        .last()
        .map(strip)
        .is_some_and(|last| last.len() >= length && last.chars().all(|c| c == marker))
}

/// Split a block's contents into its leading `:key: value` options and
/// the body after them. An indented line continues the previous option's
/// value (a trailing `\` on a piece is dropped, as in reStructuredText
/// directive options); a blank line or any other line ends the options,
/// and one blank line between the options and the body is not part of
/// the body.
fn parse_options(contents: &str) -> Result<(BTreeMap<String, String>, String), String> {
    let mut options: BTreeMap<String, String> = BTreeMap::new();
    let mut last: Option<String> = None;
    let mut lines = contents.lines().peekable();
    while let Some(&line) = lines.peek() {
        if let Some((key, value)) = option_line(line) {
            if options.contains_key(key) {
                return Err(format!("option ':{key}:' is given twice"));
            }
            options.insert(key.to_string(), piece(value));
            last = Some(key.to_string());
        } else if let Some(key) = &last
            && line.starts_with([' ', '\t'])
            && !line.trim().is_empty()
        {
            let value = options.get_mut(key).unwrap();
            let more = piece(line);
            if !more.is_empty() {
                if !value.is_empty() {
                    value.push(' ');
                }
                value.push_str(&more);
            }
        } else {
            break;
        }
        lines.next();
    }
    if !options.is_empty() && lines.peek().is_some_and(|line| line.trim().is_empty()) {
        lines.next();
    }
    let mut rest: String = lines.collect::<Vec<_>>().join("\n");
    if !rest.is_empty() {
        rest.push('\n');
    }
    Ok((options, rest))
}

fn option_line(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix(':')?;
    let (key, value) = rest.split_once(':')?;
    let valid = !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    // ":key:value" with no space is not an option line in either MyST or
    // reStructuredText; it is more likely body text.
    (valid && (value.is_empty() || value.starts_with([' ', '\t']))).then_some((key, value))
}

fn piece(text: &str) -> String {
    let text = text.trim();
    text.strip_suffix('\\')
        .unwrap_or(text)
        .trim_end()
        .to_string()
}

/// Plugin names, as used in `{name}` and `[plugins.<name>]`: lowercase
/// letters, digits, `-` and `_`, starting with a letter or digit.
fn validate_name(name: &str) -> Result<()> {
    let mut chars = name.chars();
    let valid = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    ensure!(
        valid,
        "'{name}' is not a valid plugin name (lowercase letters, digits, '-' and '_')"
    );
    Ok(())
}

/// Resolve a plugin's program, checking a path names a real file.
fn program(root: &Path, name: &str, config: &PluginConfig) -> Result<PathBuf> {
    let Some(first) = config.command.first() else {
        bail!("[plugins.{name}] has an empty command");
    };
    if !first.contains('/') {
        return Ok(PathBuf::from(first));
    }
    let path = root.join(first);
    ensure!(
        path.is_file(),
        "[plugins.{name}] command '{first}' does not exist (paths are relative to the site root)"
    );
    Ok(canonical(&path))
}

struct Outcome {
    outputs: Vec<Result<String, String>>,
    dependencies: Vec<PathBuf>,
}

/// Run a plugin over its blocks, or reuse its last output when nothing it
/// read has changed. Line numbers are not part of the cache key: editing
/// prose above a block moves it without changing what it expands to.
fn run_cached(
    root: &Path,
    name: &str,
    config: &PluginConfig,
    program: &Path,
    blocks: Vec<RequestBlock>,
) -> Result<Outcome> {
    let key = {
        let mut hasher = DefaultHasher::new();
        config.command.hash(&mut hasher);
        for block in &blocks {
            (
                block.page,
                &block.file,
                block.argument,
                block.options,
                block.body,
            )
                .hash(&mut hasher);
        }
        hasher.finish()
    };
    let slot = (root.to_path_buf(), name.to_string());
    if let Some(cached) = cache().lock().unwrap().get(&slot)
        && cached.key == key
        && cached
            .dependencies
            .iter()
            .all(|(path, print)| fingerprint(path) == *print)
    {
        return Ok(Outcome {
            outputs: cached.outputs.iter().cloned().map(Ok).collect(),
            dependencies: cached
                .dependencies
                .iter()
                .map(|(path, _)| path.clone())
                .collect(),
        });
    }

    let count = blocks.len();
    let request = Request {
        protocol: PROTOCOL,
        plugin: name,
        root: root.display().to_string(),
        blocks,
    };
    let response = run(root, config, program, &request)?;
    let outputs = check_response(response.blocks, count)?;

    let mut dependencies: Vec<PathBuf> = response
        .depends_on
        .iter()
        .map(|path| canonical(&root.join(path)))
        .collect();
    if config.command[0].contains('/') {
        dependencies.push(program.to_path_buf());
    }
    dependencies.sort();
    dependencies.dedup();

    let mut slots = cache().lock().unwrap();
    if outputs.iter().all(Result::is_ok) {
        slots.insert(
            slot,
            Cached {
                key,
                dependencies: dependencies
                    .iter()
                    .map(|path| (path.clone(), fingerprint(path)))
                    .collect(),
                outputs: outputs.iter().map(|o| o.clone().unwrap()).collect(),
            },
        );
    } else {
        slots.remove(&slot);
    }
    Ok(Outcome {
        outputs,
        dependencies,
    })
}

/// Spawn the plugin, feed it the request, and parse what it prints. The
/// plugin's stderr is passed straight through, so its warnings reach the
/// terminal.
fn run(root: &Path, config: &PluginConfig, program: &Path, request: &Request) -> Result<Response> {
    let payload = serde_json::to_vec(request)?;
    let mut child = Command::new(program)
        .args(&config.command[1..])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("starting '{}'", config.command[0]))?;
    // Write from a thread: a plugin that streams output while still
    // reading would otherwise deadlock against a full stdout pipe.
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let writer = std::thread::spawn(move || stdin.write_all(&payload));
    let output = child.wait_with_output().context("waiting for the plugin")?;
    let written = writer.join().expect("the stdin writer does not panic");
    ensure!(
        output.status.success(),
        "'{}' failed ({})",
        config.command[0],
        output.status
    );
    written.context("writing the request to the plugin")?;
    serde_json::from_slice(&output.stdout).context("the plugin's response is not valid")
}

/// Every request id must be answered exactly once, with exactly one of
/// `markdown` and `error`.
fn check_response(blocks: Vec<ResponseBlock>, count: usize) -> Result<Vec<Result<String, String>>> {
    let mut outputs: Vec<Option<Result<String, String>>> = vec![None; count];
    for block in blocks {
        ensure!(
            block.id < count,
            "the response answers block {}, but the request had only {count}",
            block.id
        );
        ensure!(
            outputs[block.id].is_none(),
            "the response answers block {} twice",
            block.id
        );
        outputs[block.id] = Some(match (block.markdown, block.error) {
            (Some(markdown), None) => Ok(markdown),
            (None, Some(error)) => Err(error),
            _ => bail!(
                "block {} in the response must have exactly one of 'markdown' and 'error'",
                block.id
            ),
        });
    }
    let missing: Vec<String> = outputs
        .iter()
        .enumerate()
        .filter(|(_, output)| output.is_none())
        .map(|(id, _)| id.to_string())
        .collect();
    ensure!(
        missing.is_empty(),
        "the response does not answer block(s) {}",
        missing.join(", ")
    );
    Ok(outputs.into_iter().map(Option::unwrap).collect())
}

/// The replacement for one fence. Output lines after the first carry the
/// container prefix the fence sat behind (`> ` in a blockquote, spaces in
/// a list item), and the output is set off by blank lines so its first
/// and last paragraphs cannot merge into the authored text around it.
fn splice_text(body: &str, range: &Range<usize>, markdown: &str) -> String {
    let line_start = body[..range.start].rfind('\n').map_or(0, |i| i + 1);
    let prefix: String = body[line_start..range.start]
        .chars()
        .map(|c| if c == '>' || c == '\t' { c } else { ' ' })
        .collect();
    let blank = prefix.trim_end();
    let mut out = String::new();
    out.push('\n');
    for line in markdown.trim_end_matches('\n').lines() {
        if line.trim().is_empty() {
            out.push_str(blank);
        } else {
            out.push_str(&prefix);
            out.push_str(line);
        }
        out.push('\n');
    }
    // The fence's own line break follows the range, ending this line.
    out.push_str(blank);
    out
}

/// How many lines precede the body in the article's source file, so a
/// body line can be reported as a file line. The body is the file's tail
/// (`read_article` only strips what comes before it).
fn body_line_offset(article: &Article) -> usize {
    std::fs::read_to_string(&article.source_file)
        .ok()
        .filter(|text| text.ends_with(&article.body))
        .map_or(0, |text| {
            text[..text.len() - article.body.len()]
                .matches('\n')
                .count()
        })
}

fn relative_file(root: &Path, article: &Article) -> String {
    article
        .source_file
        .strip_prefix(root)
        .unwrap_or(&article.source_file)
        .display()
        .to_string()
}

fn located(root: &Path, article: &Article, line: usize, message: &str) -> String {
    format!("{}:{line}: {message}", relative_file(root, article))
}

fn unknown_plugin(name: &str, plugins: &BTreeMap<String, PluginConfig>) -> String {
    if plugins.is_empty() {
        format!("'{{{name}}}' fence, but trail.toml configures no plugins")
    } else {
        let known: Vec<&str> = plugins.keys().map(String::as_str).collect();
        format!(
            "'{{{name}}}' fence names no configured plugin (configured: {})",
            known.join(", ")
        )
    }
}

fn report(errors: &[String]) -> String {
    let mut out = format!(
        "{} plugin block{} failed:",
        errors.len(),
        if errors.len() == 1 { "" } else { "s" }
    );
    for error in errors {
        let mut lines = error.lines();
        let _ = write!(out, "\n  {}", lines.next().unwrap_or_default());
        for line in lines {
            let _ = write!(out, "\n    {line}");
        }
    }
    out
}

fn fingerprint(path: &Path) -> Option<u64> {
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    Some(hasher.finish())
}

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn blocks(body: &str) -> Vec<Block> {
        let (blocks, errors) = find_blocks(body);
        assert_eq!(errors, vec![]);
        blocks
    }

    fn block_errors(body: &str) -> Vec<String> {
        find_blocks(body).1.into_iter().map(|e| e.message).collect()
    }

    #[test]
    fn a_fence_carries_its_argument_options_and_body() {
        let body = "Intro.\n\n```{kernel-doc} drivers/base/core.c\n:export:\n:doc: Device model\n\nBody text.\n```\n";
        let found = blocks(body);
        assert_eq!(found.len(), 1);
        let block = &found[0];
        assert_eq!(block.name, "kernel-doc");
        assert_eq!(block.argument, "drivers/base/core.c");
        assert_eq!(block.options["export"], "");
        assert_eq!(block.options["doc"], "Device model");
        assert_eq!(block.body, "Body text.\n");
        assert_eq!(block.line, 3);
        assert_eq!(
            &body[block.range.clone()],
            "```{kernel-doc} drivers/base/core.c\n:export:\n:doc: Device model\n\nBody text.\n```"
        );
    }

    #[test]
    fn indented_lines_continue_an_option_and_backslashes_drop() {
        let found = blocks(
            "```{kernel-doc} lib/x.c\n:functions: one two \\\n    three \\\n    four\n:no-identifiers: five\n```\n",
        );
        assert_eq!(found[0].options["functions"], "one two three four");
        assert_eq!(found[0].options["no-identifiers"], "five");
        assert_eq!(found[0].body, "");
    }

    #[test]
    fn ordinary_fences_and_samples_of_the_syntax_stay_literal() {
        let body = "```rust\nfn x() {}\n```\n\n````markdown\n```{kernel-doc} a.c\n```\n````\n\n    ```{kernel-doc} b.c\n    ```\n";
        assert_eq!(blocks(body), vec![]);
    }

    #[test]
    fn fences_in_lists_and_blockquotes_are_found() {
        let found = blocks(
            "- item\n\n  ```{p} in-list\n  :a: 1\n  ```\n\n> ```{p} quoted\n> :b: 2\n> ```\n",
        );
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].argument, "in-list");
        assert_eq!(found[0].options["a"], "1");
        assert_eq!(found[1].argument, "quoted");
        assert_eq!(found[1].options["b"], "2");
    }

    #[test]
    fn malformed_fences_are_errors() {
        assert_eq!(
            block_errors("```{p} x\n:a: 1\n:a: 2\n```\n"),
            vec!["option ':a:' is given twice"]
        );
        assert_eq!(
            block_errors("```{p x\n```\n"),
            vec!["plugin fence '{p x' has no closing '}' after its name"]
        );
        assert_eq!(
            block_errors("text\n\n```{p} x\n:a: 1\n"),
            vec!["unclosed '{p}' fence"]
        );
        assert!(block_errors("```{Bad} x\n```\n")[0].contains("not a valid plugin name"));
    }

    #[test]
    fn option_lines_need_a_space_after_the_key() {
        let found = blocks("```{p}\n:not:an option\n```\n");
        assert!(found[0].options.is_empty());
        assert_eq!(found[0].body, ":not:an option\n");
    }

    #[test]
    fn splicing_keeps_container_prefixes_and_separates_paragraphs() {
        let body = "> ```{p}\n> ```\n> after\n";
        let range = blocks(body)[0].range.clone();
        let mut out = body.to_string();
        out.replace_range(range.clone(), &splice_text(body, &range, "one\n\ntwo\n"));
        assert_eq!(out, "> \n> one\n>\n> two\n>\n> after\n");
    }

    /// A site in `<tmp>/site` whose `p` plugin is `<tmp>/plugin.sh`, a
    /// script with the given body. Articles are (file stem, body).
    fn site_with_plugin(script: &str, articles: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("site");
        let topic = root.join("demo.product/1--guide.topic");
        fs::create_dir_all(&topic).unwrap();
        fs::write(
            root.join("trail.toml"),
            "sitename = \"T\"\ntitle = \"T\"\ndescription = \"d\"\nfooter = \"f\"\n\n[plugins.p]\ncommand = [\"../plugin.sh\", \"--flag\"]\n",
        )
        .unwrap();
        fs::write(
            root.join("demo.product/trail.toml"),
            "title = \"Demo\"\nmonogram = \"D\"\ncolor = \"#3b82f6\"\ndescription = \"d\"\n",
        )
        .unwrap();
        fs::write(topic.join("trail.toml"), "title = \"Guide\"\n").unwrap();
        for (stem, body) in articles {
            fs::write(
                topic.join(format!("{stem}.md")),
                format!("---\ntitle: {stem}\ntype: concept\n---\n\n{body}"),
            )
            .unwrap();
        }
        let script_path = dir.path().join("plugin.sh");
        fs::write(&script_path, format!("#!/bin/sh\n{script}")).unwrap();
        fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).unwrap();
        dir
    }

    fn load(dir: &tempfile::TempDir) -> Result<Site> {
        let root = dir.path().join("site");
        Site::load(&root, &root.join("dist"))
    }

    fn body(site: &Site, slug: &str) -> String {
        site.articles()
            .into_iter()
            .find(|a| a.slug == slug)
            .unwrap()
            .body
            .clone()
    }

    #[test]
    fn blocks_expand_into_ordinary_body_content() {
        // The request is saved for inspection; the reply is fixed.
        let dir = site_with_plugin(
            r#"cat > ../request.json
printf '%s' '{"blocks":[{"id":0,"markdown":"Generated <a id=\"c-thing\"></a>\n\nWords."}],"depends_on":["../dep.txt"]}'
"#,
            &[("1--page", "Before.\n```{p} the-arg\n:opt: v\n```\nAfter.\n")],
        );
        fs::write(dir.path().join("dep.txt"), "x").unwrap();
        let site = load(&dir).unwrap();
        assert_eq!(
            body(&site, "page"),
            "Before.\n\nGenerated <a id=\"c-thing\"></a>\n\nWords.\n\nAfter.\n"
        );
        let index = crate::links::LinkIndex::new(&site);
        assert_eq!(index.has_anchor("/demo/guide/page", "c-thing"), Some(true));

        let request: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.path().join("request.json")).unwrap())
                .unwrap();
        assert_eq!(request["protocol"], 1);
        assert_eq!(request["plugin"], "p");
        let block = &request["blocks"][0];
        assert_eq!(block["id"], 0);
        assert_eq!(block["page"], "/demo/guide/page");
        assert_eq!(block["reference"], "demo/guide/page");
        assert_eq!(block["file"], "demo.product/1--guide.topic/1--page.md");
        assert_eq!(block["line"], 7);
        assert_eq!(block["argument"], "the-arg");
        assert_eq!(block["options"]["opt"], "v");

        let deps = dependencies(&dir.path().join("site"));
        assert!(deps.contains(&canonical(&dir.path().join("dep.txt"))));
        assert!(deps.contains(&canonical(&dir.path().join("plugin.sh"))));
    }

    #[test]
    fn reading_time_counts_the_expanded_body() {
        let dir = site_with_plugin(
            r#"cat > /dev/null
words=$(printf 'word %.0s' $(seq 1 500))
printf '{"blocks":[{"id":0,"markdown":"%s"}]}' "$words"
"#,
            &[("1--page", "```{p}\n```\n")],
        );
        let site = load(&dir).unwrap();
        let article = site
            .articles()
            .into_iter()
            .find(|a| a.slug == "page")
            .unwrap();
        assert_eq!(article.reading_minutes, 3);
    }

    #[test]
    fn block_errors_are_reported_against_the_source_line() {
        let dir = site_with_plugin(
            r#"cat > /dev/null
printf '%s' '{"blocks":[{"id":0,"error":"no such file"},{"id":1,"markdown":"ok"}]}'
"#,
            &[(
                "1--page",
                "Para.\n\n```{p} missing.c\n```\n\n```{p} fine.c\n```\n",
            )],
        );
        let error = format!("{:#}", load(&dir).unwrap_err());
        assert!(
            error.contains("demo.product/1--guide.topic/1--page.md:8: p: no such file"),
            "{error}"
        );
    }

    #[test]
    fn unknown_plugins_fail_the_load() {
        let dir = site_with_plugin("cat > /dev/null\n", &[("1--page", "```{nope} x\n```\n")]);
        let error = format!("{:#}", load(&dir).unwrap_err());
        assert!(
            error.contains(
                "1--page.md:6: '{nope}' fence names no configured plugin (configured: p)"
            ),
            "{error}"
        );
    }

    #[test]
    fn broken_responses_fail_the_load() {
        for (script, expected) in [
            ("cat > /dev/null\nexit 3\n", "failed (exit status: 3)"),
            ("cat > /dev/null\necho not-json\n", "response is not valid"),
            (
                "cat > /dev/null\necho '{\"blocks\":[]}'\n",
                "does not answer block(s) 0",
            ),
            (
                "cat > /dev/null\necho '{\"blocks\":[{\"id\":0}]}'\n",
                "exactly one of 'markdown' and 'error'",
            ),
        ] {
            let dir = site_with_plugin(script, &[("1--page", "```{p}\n```\n")]);
            let error = format!("{:#}", load(&dir).unwrap_err());
            assert!(error.contains(expected), "{script}: {error}");
        }
    }

    #[test]
    fn output_is_reused_until_a_dependency_or_block_changes() {
        let dir = site_with_plugin(
            r#"cat > /dev/null
echo run >> ../runs
printf '%s' '{"blocks":[{"id":0,"markdown":"out"}],"depends_on":["../dep.txt"]}'
"#,
            &[("1--page", "```{p}\n```\n")],
        );
        let runs = || {
            fs::read_to_string(dir.path().join("runs"))
                .unwrap()
                .lines()
                .count()
        };
        fs::write(dir.path().join("dep.txt"), "one").unwrap();
        load(&dir).unwrap();
        // Prose edits move the block without changing it.
        let page = dir
            .path()
            .join("site/demo.product/1--guide.topic/1--page.md");
        fs::write(
            &page,
            "---\ntitle: page\ntype: concept\n---\n\nNew prose.\n\n```{p}\n```\n",
        )
        .unwrap();
        let site = load(&dir).unwrap();
        assert_eq!(runs(), 1);
        assert!(body(&site, "page").contains("out"));

        fs::write(dir.path().join("dep.txt"), "two").unwrap();
        load(&dir).unwrap();
        assert_eq!(runs(), 2);

        fs::write(
            &page,
            "---\ntitle: page\ntype: concept\n---\n\n```{p} changed\n```\n",
        )
        .unwrap();
        load(&dir).unwrap();
        assert_eq!(runs(), 3);
    }

    #[test]
    fn plugins_without_blocks_never_run() {
        let dir = site_with_plugin("echo run >> ../runs\n", &[("1--page", "Plain.\n")]);
        load(&dir).unwrap();
        assert!(!dir.path().join("runs").exists());
    }

    #[test]
    fn a_missing_program_path_fails_the_load() {
        let dir = site_with_plugin("", &[("1--page", "Plain.\n")]);
        fs::remove_file(dir.path().join("plugin.sh")).unwrap();
        let error = format!("{:#}", load(&dir).unwrap_err());
        assert!(
            error.contains("[plugins.p] command '../plugin.sh' does not exist"),
            "{error}"
        );
    }
}
