---
title: Articles and frontmatter
type: how-to
description: Every frontmatter key an article may carry — title, type, description, related, inline_ref, updated and reading_minutes — what each one actually does, and how reading times and dates roll up the tree.
related:
  - trail/writing/links-and-references
  - trail/reference/frontmatter
  - trail/structuring-a-site/topics-and-folders
  - trail/building/build-and-serve
---

An article is a single `.md` file named `<order>--<slug>.md`: a YAML frontmatter block, then markdown.

```markdown
---
title: Quick start
type: how-to
description: Build and package a tiny "hello" program with pekit, from an empty directory.
related:
  - pekit/getting-started/what-is-pekit
  - pekit/recipes/anatomy
---

This page takes you from an empty directory to a working `.peipkg`…
```

The frontmatter is required and must be the first thing in the file: a line of exactly `---`, the YAML, and a closing `---`. A file that does not start with `---`, or whose block is never closed, is a build error. So is any key not listed below — a typo'd `descripton:` fails the build rather than silently doing nothing.

## The keys

| Key | Required | |
|---|---|---|
| `title` | yes | The page's name: its `h1`, its breadcrumb, its sidebar entry, its search-result heading, its `og:title`. |
| `type` | in topics | The learn taxonomy — `concept`, `how-to`, `reference` on this site. **An error inside a [book](~trail/structuring-a-site/books).** |
| `description` | no | One sentence. Becomes the page's meta description and social-preview text. |
| `related` | no | Page references shown as a "Related Content" list at the foot of the page. |
| `inline_ref` | no | Phrases that auto-link to this page wherever prose states them. |
| `updated` | no | An ISO `YYYY-MM-DD` date. |
| `reading_minutes` | no | Overrides the estimated reading time. |

### title

The only key every article must have, in topics and books alike. It is not derived from the slug — unlike a topic or chapter title, an article states its own.

### type

Required on every article in a topic, and rejected on every article in a book.

Trail does not constrain the value or display it on the page; it is carried through to `site.json` for tooling that wants to filter by it. What it is really for is the author: deciding whether a page is a **concept** (explaining how something works), a **how-to** (walking through a task) or a **reference** (exhaustive, looked-up rather than read) is a genuinely useful discipline, and pages that cannot answer the question are usually two pages.

Inside a book the question is already answered — a specification is read in order — so trail rejects the key rather than let it sit there meaning nothing.

### description

Optional, but write one. It becomes:

- the page's `<meta name="description">` and `og:description`, which is what search engines and chat apps show;
- the standfirst line in the page's [markdown mirror](~trail/building/the-output-surface);
- the `description` field in `site.json`.

It is not displayed on the page itself, and it is not the in-site search snippet — Pagefind builds those from the body text around the match.

Build with [`--strict`](~trail/reference/cli) to make a missing description a build failure. Alias pages are exempt, since the original carries it.

### related

A list of page references, in the same grammar body links use but **without** the leading `~`:

```yaml
related:
  - pekit/running/invocation
  - peios/package-management/overview
```

They render as a "Related Content" list under the article, and they appear in the page's markdown mirror and in `site.json`. Resolution is as strict as a body link: an ambiguous reference always fails the build, and a missing one fails unless `--allow-dangling-links` is set, in which case it warns and drops from the list.

### inline_ref

Phrases that should become links to this article wherever they appear in the site's prose:

```yaml
inline_ref:
  - PCDS GUID
  - PCDS GUIDs
```

Every occurrence in ordinary prose, anywhere on the site, links here — except on this page itself. Phrases are claimed site-wide and exclusively; two pages claiming the same phrase is a build error. See [Inline references](~trail/writing/inline-references).

### updated

An ISO date, and validated as one — `2026-05-01`, not `May 2026` and not `01/05/2026`.

```yaml
updated: 2026-05-01
```

It shows in the page's meta line, and it becomes the page's `<lastmod>` in `sitemap.xml`. It is deliberately manual: file modification times are noise (a whitespace fix is not an update, and a checkout rewrites them all), and trail does not read git history. An article with no `updated:` simply does not show one.

### reading_minutes

Trail estimates reading time from the body's word count at 200 words a minute, rounded up, with a floor of one minute. Code blocks count — they are read, just differently.

Set `reading_minutes:` when the estimate is wrong: a dense reference table reads far slower than its word count suggests, and a page that is mostly a long code listing far faster.

```yaml
reading_minutes: 30
```

## Roll-ups

Reading times and dates add up the tree. A subfolder totals its articles; a topic totals its articles and subfolders; a book totals its chapters; an anthology totals its topics and books; a product totals everything in it.

- **Reading time** is the sum, so a book's cover can say how long the whole book takes and a topic card can say how long the topic is.
- **`updated`** is the *most recent* date anywhere beneath, so a container reflects its freshest page.

Roll-ups are computed after `.link` aliases are resolved, so an aliased article counts in the totals of the place it was linked into.

## The body

Everything after the closing `---` is [CommonMark](https://commonmark.org) with GitHub extensions: tables, strikethrough, footnotes, and `> [!NOTE]`-style [admonitions](~trail/writing/admonitions-tabs-and-diagrams). On top of that, trail adds:

- [`~page` references](~trail/writing/links-and-references) — link by page identity instead of URL.
- [Co-located images](~trail/writing/images) — `![alt](diagram.svg)` beside the article, with dimensions filled in and captions turned into figures.
- [Syntax-highlighted code blocks](~trail/writing/code-blocks) with copy buttons.
- [Tab groups and mermaid diagrams](~trail/writing/admonitions-tabs-and-diagrams).
- [Inline references, `§` section citations and RFC-2119 keyword highlighting](~trail/writing/inline-references).

A few things happen to headings automatically: `h2` and `h3` become the "On this page" list, every heading gets a stable id and a permalink anchor, and inside a book they carry their section numbers as real text. Ids come from the heading text, with a numeric suffix if two headings on a page would otherwise collide.

Tables are wrapped in their own horizontally-scrolling container, so a wide table never makes the whole page scroll sideways.

## Naming and ordering

The filename is not decoration:

```text
200--quick-start.md
 │        └── the URL segment, and what you write in a ~link
 └── the sort key among siblings — never shown, never in a URL
```

Two files sharing an order in the same directory is an error. Numbering in hundreds leaves room to insert later; inside a [book](~trail/structuring-a-site/books) the prefix is the published section number, so number from 1 there instead.

`print` is a reserved slug — every container publishes a `/print` page — so `100--print.md` is rejected.
