---
title: Directory and file names
type: reference
description: The complete naming grammar — order prefixes, slugs, type suffixes, appendix orders — plus exactly what each kind of directory may contain, the reserved names, and the sorting and duplicate rules.
related:
  - trail/getting-started/anatomy-of-a-site
  - trail/reference/trail-toml
  - trail/reference/frontmatter
  - trail/structuring-a-site/books
---

Trail derives the whole site from the tree, so every name in it is meaningful and every unrecognised name is a build error. This page is the exhaustive grammar. [Anatomy of a site](~trail/getting-started/anatomy-of-a-site) is the same material as a walkthrough.

## The name shapes

| Shape | Example | What |
|---|---|---|
| `<slug>.product` | `pekit.product` | A product. Site root only; no order prefix. |
| `<order>--<slug>.antho` | `1--security.antho` | An anthology. |
| `<order>--<slug>.topic` | `4--reference.topic` | A topic. |
| `<order>--<slug>.book` | `2--pgss.book` | A book. |
| `<order>--<slug>.shelf` | `3--tooling.shelf` | A shelf. |
| `<order>--<slug>` | `300--tokens` | A topic subfolder or a book chapter. No suffix. |
| `<order>--<slug>.md` | `100--sids.md` | An article. |
| `<order>--<slug>.link` | `3--faq.link` | A [link alias](~trail/structuring-a-site/link-aliases). |
| `<slug>.<ext>` | `logon-flow.svg` | A co-located [image](~trail/writing/images). No order prefix. |
| `trail.toml` | | Configuration, at every level. |

### `<order>`

A non-negative integer, parsed as written. `1`, `100` and `007` are all valid and all mean their numeric value.

- It is a **sort key only** — never shown, never part of a URL — **except inside a [book](~trail/structuring-a-site/books)**, where it *is* the published section number.
- Gaps are fine and normal.
- **Two siblings sharing an order is an error**, since their relative order would then depend on nothing visible.
- Anything not parseable as an integer is an error: no `1.5`, no `-1`, no `first`.

### `a<N>` — appendix orders

**Inside a book only.** An order written `a1`, `a2`, … marks an appendix entry, which is lettered rather than numbered and sorts after every numbered sibling at its level.

- Letters are bijective base 26: `a1` → A, `a26` → Z, `a27` → AA.
- The letter comes from the number, not from position, so gaps show through here too.
- `a0` is an error: appendix orders start at `a1`.
- Outside a book, an `a<N>--` prefix is just a non-numeric order prefix, and therefore an error.

### `<slug>`

Lowercase kebab-case:

```text
ASCII lowercase letters, digits and hyphens
not empty
not starting or ending with a hyphen
```

`getting-started`, `pgss`, `rfc-2119` are fine. `Getting_Started`, `-draft`, `tokens/`, `café` are not.

The slug is the URL segment, and it is what you write in a [`~link`](~trail/writing/links-and-references).

### `<kind>`

Exactly one of `antho`, `topic`, `book`, `shelf`. Any other suffix is an error, as is a grouping directory with no suffix at all in a place where one is expected.

## What each directory may contain

Anything not listed is a build error. Entries whose name starts with `.` are skipped everywhere, so `.git`, `.DS_Store` and editor droppings are invisible to trail.

| Directory | Subdirectories | Files |
|---|---|---|
| **Site root** | `*.product`, the output dir | `trail.toml`; whatever `favicon`, `custom_css`, `head_html`, `passthrough` and `[[merge]]` name |
| **Product** | `.antho`, `.topic`, `.book`, `.shelf` | `trail.toml` |
| **Anthology** | `.antho`, `.topic`, `.book`, `.shelf` | `trail.toml` |
| **Shelf** | `.antho`, `.topic`, `.book` — never another `.shelf` | `trail.toml` |
| **Topic** | `<order>--<slug>` subfolders | `trail.toml`, `.md`, `.link`, images |
| **Topic subfolder** | *(none — one level only)* | `trail.toml`, `.md`, `.link`, images |
| **Book** | `<order>--<slug>` chapters | `trail.toml`, `.md`, `.link`, images |
| **Book chapter** | `<order>--<slug>` chapters, to any depth | `trail.toml`, `.md`, `.link`, images |

The dividing line: **directories that hold groupings hold no files but `trail.toml`; directories that hold articles hold no grouping directories.**

## `trail.toml` by level

| Level | Required? | Keys |
|---|---|---|
| Site root | **required** | see [the `trail.toml` reference](~trail/reference/trail-toml) |
| Product | **required** | `title`, `monogram`, `color`, `description`, `inline_ref` |
| Anthology | **required** | `title`, `description`, `inline_ref` |
| Book | **required** | `title`, `description`, `short`, `inline_ref` |
| Topic | optional | `title`, `inline_ref` |
| Topic subfolder | optional | `title`, `inline_ref` |
| Book chapter | optional | `title`, `inline_ref` |
| Shelf | optional | `title` only — **`inline_ref` is an error** |

Unknown keys are errors at every level. Where `title` is optional and absent, it is derived from the slug: hyphens to spaces, in title case (`the-two-gates` → "The Two Gates", `developing-for-peios` → "Developing for Peios"). [Derived titles](~trail/reference/trail-toml#derived-titles) has the casing rule.

## Reserved names

| Name | Where | Why |
|---|---|---|
| `print` | article slugs | every container publishes a `/print` page |
| `assets` | product slugs | collides with `/assets` in the output |
| `pagefind` | product slugs | collides with `/pagefind` in the output |

## Images

A co-located image is `<slug>.<ext>` where the stem is a valid slug and the extension is **lowercase** and one of:

```text
png   jpg   jpeg   gif   svg   webp   avif
```

Anything else in an article directory falls through to the "unexpected file" error.

## Sorting

Siblings sort by order, ascending. Inside a book, appendix entries sort after every numbered entry at the same level, whatever their numbers.

Products are the exception: no order prefix, so they sort by the root config's `featured` list first (in that list's order), then the rest by title.

## Duplicates

Within one container, both order and slug must be unique:

```text
items 'concepts' and 'guides' share order 1
duplicate article slug 'overview'
```

The noun is `item` for a container's mixed children (topics, books, anthologies, shelves), and `article` inside a topic subfolder or where only articles can sit. Slugs are reported without their order prefixes.

Shelf contents are checked **flattened into their parent**, because a shelf adds no URL segment — two shelves in one product cannot both hold a `getting-started` topic, since both would want the same URL.

## URLs

A page's URL is the product slug, then the slug of every container between it and the page, with two exceptions: shelves contribute nothing, and order prefixes contribute nothing.

```text
peios.product/1--security.antho/2--tokens.topic/300--issuing/100--flow.md
  →  /peios/security/tokens/issuing/flow
```

Pages are written to disk as `<path>/index.html`.
