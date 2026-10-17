---
title: Checking Style
type: how-to
description: How trail lint checks a site's sources against the writing rules the site turns on, covering configuring rules and word lists, what counts as prose, each built-in rule, warnings, and suppressing a rule for a line, a stretch, a page, or a book.
related:
  - trail/reference/cli
  - trail/reference/trail-toml
  - trail/writing/articles-and-frontmatter
---

`trail lint` reads a site's sources and reports each breach of its writing
rules, with the file, line, and column. Every rule is off until the site turns
it on, and every word list is the site's own, so Trail holds no house style of
its own.

The lint is a separate command. A build never runs it, so a site that doesn't
use it is unaffected.

```sh
trail lint path/to/site
```

Findings and totals go to stderr:

```text
trail: guide.product/1--basics.topic/100--intro.md:12:41: "e.g.": use "for example" [words]
trail: warning: guide.product/1--basics.topic/100--intro.md:30:18: add a serial comma before the last item [serial-comma]
trail: 1 error, 1 warning
```

The exit status is `1` if any breach is an error, and `0` otherwise. The
[command-line reference](~trail/reference/cli) has the details.

## Complete the task

1. Choose the writing rules and word lists for the site.

2. Add the `[lint]` configuration shown below to its root `trail.toml`.

3. Run `trail lint <root>` and read findings on stderr.

4. Correct the sources or add a justified, narrowly scoped suppression.

5. Run the check again. An exit status of 0 means no findings classified as
   errors remain.

## Turning rules on

Rules are configured in a `[lint]` table in the root `trail.toml`, below every
plain key:

```toml
[lint]
rules = ["words", "spelling", "em-dash", "heading-levels", "line-length"]
line_length = 78
proper_nouns = ["Peios", "Registry Editor", "netd"]

[[lint.words]]
match = ["e.g."]
use = "for example"

[[lint.words]]
match = ["simply", "just", "of course"]
message = "don't condescend; cut it"

[[lint.words]]
rule = "spelling"
match = ["colour*", "behaviour*", "catalogue"]
message = "use the US spelling"
```

The field and rule reference is in `trail-lint(5)`.

A malformed `[lint]` table fails `trail build` too, so a typo can't hide until
someone runs the lint.

## Word lists

Each `[[lint.words]]` table bans some words or phrases:

The field and rule reference is in `trail-lint(5)`.

## What counts as prose

The lint reads sources as written, without building them, so nothing a build
generates is checked. Prose is the text a reader sees, so these are never
checked as words:

- Code spans and code blocks;
- Raw HTML;
- Link destinations and URLs;
- Frontmatter keys.

An article's frontmatter `title` and `description`, and every `trail.toml` and
link file's `title` and `description`, are prose. The root `trail.toml` isn't
checked.

## The built-in rules

Each rule's name is what turns it on, appears in its reports, and suppresses
it.

The field and rule reference is in `trail-lint(5)`.

Some rules are heuristics. They aim never to report what isn't a breach, and
so they miss some that are.

- **`title-case` and `heading-case`** leave alone any word in `proper_nouns`,
  and any word whose casing is its own: an acronym ("GXWI", "SIDs"), or a name
  with a capital or digit inside it ("IPv6"). A word after a colon starts a
  subtitle and can be capitalized. A lowercase name such as "netd" is fine at
  the start of a title or heading once it is in `proper_nouns`.
- **`line-length`** exempts tables, code blocks, raw HTML, frontmatter, and
  link reference definitions. A line is also exempt when it fits once its link
  destinations and URLs are taken out.
- **`hex-case`** looks inside code spans, unlike every other rule, because a
  hexadecimal value in prose is usually written as code: `0x1F`. Code blocks
  follow the code's own style and are not checked.
- **`serial-comma`** counts a comma-separated run as a list only when every
  item but the first is at most three words. It also skips a run that starts
  with an introductory clause or phrase ("If the key exists, read it and write
  it"), contrasts ("`x`, not `y` or `z`"), gives examples ("such as A or B"),
  or quotes. A missed list is better than a wrong report, so it's a warning.

## Suppressing a rule

When a page breaks a rule on purpose, such as a style guide quoting the words
it bans, suppress the rule rather than turning it off for the whole site.

For one line, put a comment on the line before it:

```markdown
<!-- lint-disable-next-line words -->
- Don't: "In this article, we will discuss logon sessions."
```

For a stretch of a page, put comments around it:

```markdown
<!-- lint-disable words -->
Never write a lowercase "may" in prose.
<!-- lint-enable words -->
```

Name one or more rules, or none for every rule. A comment can sit in a
blockquote (`> <!-- … -->`) or be indented into a list item.

Put the comments between blocks, not inside a paragraph. A comment line ends
the paragraph it interrupts, which changes the page. The comments are
invisible on the page, but the page's
[Markdown mirror](~trail/building/the-output-surface) shows them, because
mirrors show the source as written.

For a whole page or a whole book, name its path in `[[lint.ignore]]`:

```toml
[[lint.ignore]]
path = "guide.product/3--conventions.book"
rules = ["hex-case"]
```

The path is relative to the site root, and covers a file or everything under a
directory. With no `rules`, every rule is ignored there.
