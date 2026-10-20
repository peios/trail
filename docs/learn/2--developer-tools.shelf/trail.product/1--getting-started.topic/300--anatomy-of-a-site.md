---
title: Anatomy of a Site
type: concept
description: The whole tree, level by level — how directory and file names become URLs, ordering and titles, which trail.toml lives where, and what can legally sit at each level.
related:
  - trail/getting-started/what-is-trail
  - trail/structuring-a-site/products
  - trail/reference/directory-names
  - trail/reference/trail-toml
---

<span id="what-may-sit-where"></span>

A trail site is a directory tree in which every name means something. This
page walks that tree from the root down, and explains the naming grammar all
of it shares. [Directory and file names](~trail/reference/directory-names) is
the same material as an exhaustive table;
[Structuring a site](~trail/structuring-a-site/products) covers each kind of
container in depth.

## The whole shape

```text
learn/                                  the site root
├── trail.toml                           the site config — required
├── dist/                                the build output — trail's to manage
│
├── peios.product/                       a product  →  /peios
│   ├── trail.toml                       required: title, monogram, color, description
│   ├── 1--security-fundamentals.antho/  an anthology  →  /peios/security-fundamentals
│   │   ├── trail.toml                   required: title, description
│   │   ├── 1--identity.topic/           a topic  →  /peios/security-fundamentals/identity
│   │   │   ├── trail.toml               optional: title, inline_ref
│   │   │   ├── 100--sids.md             an article  →  …/identity/sids
│   │   │   ├── 200--principals.md
│   │   │   ├── 300--tokens/             a subfolder  →  …/identity/tokens/…
│   │   │   │   ├── trail.toml           optional
│   │   │   │   └── 100--issuing.md      →  …/identity/tokens/issuing
│   │   │   └── diagram.svg              an image, published beside its article
│   │   └── 2--pgss.book/                a book  →  /peios/security-fundamentals/pgss
│   │       ├── trail.toml               required: title, description; optional: short
│   │       ├── 1--scope.md              §1  →  …/pgss/scope
│   │       ├── 2--model/                a chapter, §2
│   │       │   ├── trail.toml           optional
│   │       │   └── 1--objects.md        §2.1
│   │       └── a1--glossary.md          Appendix A
│   └── 2--tooling.shelf/                a shelf — a heading, not a URL segment
│       └── 1--pekit.topic/              →  /peios/pekit   (the shelf adds nothing)
│
├── pekit.product/                       another product  →  /pekit
│
└── 1--tools.shelf/                      a root shelf — a heading over products
    └── provium.product/                 →  /provium   (the shelf adds nothing)
```

A root anthology can replace a product when separate product branding is
not needed. It uses `<slug>.antho` and requires a title and description.
A root shelf groups products and root anthologies under a heading in the
Documentation menu and on the front page.

Everything below the root is optional. A site with one product, one topic, and
one article is a complete site; this site's `trail.product` is exactly that
shape, four levels shallower than `peios.product`.

## The naming grammar

These name shapes appear throughout the tree.

| Shape | Example | Where |
| ------------------------ | ------------------------- | -------------------------------------------------------------------- |
| `<slug>.product` | `pekit.product` | Only at the site root or on a root shelf. |
| `<slug>.antho` | `peios.antho` | An anthology at the site root or on a root shelf. |
| `<order>--<slug>.<kind>` | `2--writing.topic` | Anthologies, topics, books, shelves (root shelves too). |
| `<order>--<slug>` | `300--tokens`, `2--model` | Topic subfolders and book chapters — no suffix. |
| `<order>--<slug>.md` | `100--sids.md` | Articles. |
| `<order>--<slug>.link` | `3--faq.link` | Aliases; see [Link aliases](~trail/structuring-a-site/link-aliases). |

The parts:

- **`<order>`** is a non-negative integer used only for sorting among
  siblings. It never appears in a URL or a title. Gaps are fine and normal —
  numbering articles `100`, `200`, `300` leaves room to insert. Two siblings
  sharing an order is an error, because the resulting page order would depend
  on nothing you can see.
- **`<slug>`** is lowercase kebab-case: ASCII lowercase letters, digits and
  hyphens, not starting or ending with a hyphen. It becomes the URL segment,
  and it is what you write in a `~link`.
- **`<kind>`** is `antho`, `topic`, `book`, or `shelf`.

Inside books, an order can also be written `a<N>--` (`a1--glossary.md`). Those
are **appendices**: lettered `A`, `B`, `C` instead of numbered, and sorted
after every numbered sibling at their level. See
[Books](~trail/structuring-a-site/books).

## URLs

A page's URL starts with its root collection slug, followed by each
container between that collection and the page — with two exceptions.

```text
peios.product/1--security-fundamentals.antho/1--identity.topic/100--sids.md
  →  /peios/security-fundamentals/identity/sids
```

The exceptions:

- **A shelf contributes no segment.** It is a heading on its parent's page,
  nothing more. A topic inside `2--tooling.shelf/` lives at `/peios/<topic>`,
  exactly as if the shelf were not there.
- **Order prefixes contribute nothing.** `1--identity.topic` gives the segment
  `identity`.

Which containers have a page of their own is worth knowing, because it decides
what you can link to:

| Item | Has a page | Links to it land on |
| --------------- | ------------------------------ | -------------------------------------------- |
| Site | `/` | — |
| Product | `/pekit` | its own landing page |
| Anthology | `/peios/security-fundamentals` | its own landing page |
| Book | `/peios/…/pgss` (the cover) | the cover |
| Article | its own URL | itself |
| Topic | no page | _(topics are not link targets)_ |
| Topic subfolder | no page | _(not a link target, except from a `.link`)_ |
| Book chapter | no page | _(not a link target)_ |
| Shelf | no page | _(not a link target)_ |

Topics, subfolders, and chapters are real structure — they appear in sidebars,
cards and contents lists, and links from those surfaces open their first
article — but they have no page you can address, so a `~link` cannot point at
one. Point at the article you actually mean.

## Titles

Titles come from three places, in order of precedence:

1. **An article's `title:` frontmatter**, which is required.
2. **A container's `trail.toml`**, where the `title` key is required for
   products, anthologies and books, and optional for topics, subfolders,
   chapters and shelves.
3. **The slug** becomes title case after hyphens become spaces:
   `the-two-gates` becomes
   "The Two Gates", and `developing-for-peios` becomes "Developing for Peios".

The derived form is good enough surprisingly often; write a `trail.toml` when
it is not — acronyms and casing it cannot guess ("Peiso", "The Console"), or
when the container needs `inline_ref` phrases.

## What can sit where

Trail rejects anything it does not recognize, so it is worth knowing what each
level tolerates.

| Directory | Can contain |
| ----------------- | ---------------------------------------------------------------------------------- |
| The site root | `trail.toml`, `*.product/`, `*.antho/`, `.shelf/`, the output directory, files named by the config |
| A product | `trail.toml`, `.antho/`, `.topic/`, `.book/`, `.shelf/` |
| An anthology | `trail.toml`, `.antho/`, `.topic/`, `.book/`, `.shelf/` |
| A shelf | `trail.toml`, the same as its parent minus shelves — a shelf never holds a shelf |
| A root shelf | `trail.toml`, `*.product/`, `*.antho/` |
| A topic | `trail.toml`, `.md` articles, `<order>--<slug>/` subfolders, `.link` files, images |
| A topic subfolder | `trail.toml`, `.md` articles, `.link` files, images |
| A book | `trail.toml`, `.md` articles, `<order>--<slug>/` chapters, `.link` files, images |
| A book chapter | `trail.toml`, `.md` articles, `<order>--<slug>/` chapters, `.link` files, images |

Only the article-holding levels take files: `.md` articles, `.link` aliases
and images can sit in a topic, a subfolder, a book or a chapter, and nowhere
else. A product, anthology or shelf directory holds `trail.toml` and
subdirectories, full stop.

Images sit beside the articles that use them and are published at the URL of
the directory they were found in; see [Images](~trail/writing/images).

Two slugs are reserved: `print` (every container publishes a `/print` page)
can not be an article slug, and `assets` and `pagefind` can not be product
slugs.

## Ordering

Siblings sort by their `<order>` prefix, ascending. Within a book, appendix
entries (`a<N>--`) sort after every numbered entry at the same level, and they
are lettered rather than numbered.

Root collections have no order prefix. They sort by the root
config's `featured` list first, in that list's order, then everything else by
title. Root shelves sort by their prefix, after the collections outside any
shelf.

## Where to go next

- [Products](~trail/structuring-a-site/products) — the product level in depth,
  and when to add another one.
- [Anthologies and shelves](~trail/structuring-a-site/anthologies-and-shelves)
  — grouping a large product.
- [Topics and folders](~trail/structuring-a-site/topics-and-folders) — the
  everyday container.
- [Books](~trail/structuring-a-site/books) — numbered, ordered documents with
  chapters and appendices.
