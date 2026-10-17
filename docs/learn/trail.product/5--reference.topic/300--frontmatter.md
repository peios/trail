---
title: Frontmatter reference
type: reference
description: Every key an article's YAML frontmatter may carry, its type, whether it is required, what it feeds, and how it is validated — with the differences between topic articles and book articles set out.
related:
  - trail/writing/articles-and-frontmatter
  - trail/reference/trail-toml
  - trail/writing/inline-references
  - trail/reference/cli
---

Every article begins with a YAML frontmatter block: a line of exactly `---`, the keys, and a closing `---`. It must be the first thing in the file. **Unknown keys are build errors.**

```markdown
---
title: Command-line reference
type: reference
description: Every pekit flag with its value form, the capability matrix, and exit codes.
related:
  - pekit/running/invocation
  - pekit/reference/recipe-format
inline_ref:
  - pekit CLI
updated: 2026-05-01
reading_minutes: 25
---
```

## The keys

| Key | Type | Topic article | Book article |
|---|---|---|---|
| `title` | string | **required** | **required** |
| `type` | string | **required** | **error** |
| `description` | string | optional | optional |
| `related` | array of strings | optional | optional |
| `inline_ref` | array of strings | optional | optional |
| `updated` | ISO date | optional | optional |
| `reading_minutes` | integer | optional | optional |

### `title`

The page's name: its `h1`, breadcrumb, sidebar entry, search-result heading and `og:title`. Never derived — an article always states its own.

### `type`

The learn taxonomy. Required on articles in topics; **an error on articles in books**, where the question it answers is already settled by the document's order.

Trail does not constrain the value and does not display it on the page. It is carried into `site.json` as the article's `type`. This site uses `concept`, `how-to` and `reference`.

### `description`

One sentence. It feeds:

- `<meta name="description">` and `og:description`;
- the standfirst line of the page's [markdown mirror](~trail/building/the-output-surface);
- the `description` field in `site.json`.

It is not shown on the page, and it is not the in-site search snippet — Pagefind builds those from body text.

[`--strict`](~trail/reference/cli) fails the build on any article without one. [Alias pages](~trail/structuring-a-site/link-aliases) are exempt.

### `related`

Page references in the [`~` grammar](~trail/writing/links-and-references), **without the leading `~`**:

```yaml
related:
  - pekit/running/invocation
  - peios/package-management/overview
```

They render as a "Related Content" list under the article, appear in the markdown mirror, and become a list of resolved paths in `site.json`.

Resolution is as strict as a body link: **ambiguous references always fail the build**; missing ones fail unless `--allow-dangling-links` downgrades them to a warning, in which case they are dropped from the list.

### `inline_ref`

Phrases that should link to this article wherever they appear in the site's prose:

```yaml
inline_ref:
  - PCDS GUID
  - PCDS GUIDs
```

Matching is literal, whole-word and longest-first, and skips headings, links, image alt text, code, and the claiming page itself. A phrase belongs to exactly one declarer site-wide; a collision is a build error, as is an empty phrase, one with leading or trailing whitespace, or one containing `§`. Alias pages claim nothing. See [Inline references](~trail/writing/inline-references).

### `updated`

An ISO `YYYY-MM-DD` date, validated as such:

```text
updated: '2026/05/01' is not an ISO date (expected YYYY-MM-DD)
```

It appears in the page's meta line and becomes the page's `<lastmod>` in `sitemap.xml`. Containers take the most recent date beneath them.

Deliberately manual: file modification times are noise, and trail does not read git history.

> [!TIP]
> Both components must be zero-padded to two digits: `2026-05-01`, never `2026-5-1`. No quoting is needed — an unquoted date works.

### `reading_minutes`

An integer overriding the estimate, which is otherwise the body's word count at 200 words a minute, rounded up, with a floor of 1. Code blocks count toward the word count.

Containers sum their children's values, so a book cover reports the whole book.

## Errors

| Message | Cause |
|---|---|
| `… has no frontmatter (must start with '---')` | The file does not open with `---`. |
| `… has unterminated frontmatter` | No closing `---`. |
| `parsing frontmatter of …` | Invalid YAML, a missing required key, or an unknown key. |
| `updated: '…' is not an ISO date (expected YYYY-MM-DD)` | A malformed date. |

Inside a book, `type:` produces an unknown-key error, since `type` is not part of the book article schema.

## Frontmatter and `.link` aliases

An [alias page](~trail/structuring-a-site/link-aliases) has no frontmatter of its own — it renders the target's body. It takes its title from the `.link` file (or from its own slug), its canonical URL from the original, and it claims no `inline_ref` phrases. Inside a book it also loses `type` and `description`, as every book entry does.
