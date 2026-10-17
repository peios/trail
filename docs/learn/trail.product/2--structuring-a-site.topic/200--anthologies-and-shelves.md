---
title: Anthologies and shelves
type: concept
description: Two ways to group a product's contents — an anthology is a second landing page with its own URL, a shelf is a heading on the page above it. When to reach for each, and how deep they nest.
related:
  - trail/structuring-a-site/products
  - trail/structuring-a-site/topics-and-folders
  - trail/getting-started/anatomy-of-a-site
  - trail/reference/directory-names
---

Once a product has more than a handful of topics, its landing page becomes a wall of cards. Trail offers two ways to break that up, and the difference between them is exactly one thing: **whether the grouping gets a page of its own**.

| | URL segment | Page | Sidebar | Nests |
|---|---|---|---|---|
| **Anthology** (`.antho`) | yes | its own landing page | — | in products and anthologies |
| **Shelf** (`.shelf`) | no | no — a heading on its parent's page | — | in products and anthologies, never in a shelf |

Reach for a shelf when the topics belong together but readers should still see them all at once. Reach for an anthology when the group is big enough to deserve its own front door.

## Anthologies

An anthology is a directory named `<order>--<slug>.antho`. It has a landing page at its own URL, listing everything inside it exactly as a product page lists its own children.

```text
peios.product/
├── trail.toml
├── 1--security-fundamentals.antho/     →  /peios/security-fundamentals
│   ├── trail.toml
│   ├── 1--identity.topic/              →  /peios/security-fundamentals/identity
│   └── 2--pgss.book/                   →  /peios/security-fundamentals/pgss
└── 2--using-peios.antho/               →  /peios/using-peios
```

Its `trail.toml` requires two keys:

```toml
# 1--security-fundamentals.antho/trail.toml
title = "Security fundamentals"
description = "How Peios decides who may do what: identity, tokens, and the two gates every request passes."
```

`title` is the heading and the breadcrumb; `description` is the standfirst on the page, the text on the card that links to it from the product page, and the entry in `llms.txt`. The optional `inline_ref` key claims phrases pointing at this anthology's page; see [Inline references](~trail/writing/inline-references).

An anthology may contain anthologies, topics, books and shelves — the same set a product may contain. Anthologies nesting inside anthologies is legal and unbounded, and each level adds a URL segment:

```text
/peios/developing-for-peios/apis/filesystem/open
      └─ anthology ──────┘ └ antho ┘ └ topic ─┘ └ article
```

In practice two levels is plenty. Deeper than that and readers stop being able to guess where anything lives.

### The anthology page

The page mirrors a product page one level down: a breadcrumb trail back to the product (and through any parent anthologies), the title, the description, and a "Topics" section of cards. Nested anthologies and books get a description card; topics get a card listing their articles.

## Shelves

A shelf is a directory named `<order>--<slug>.shelf`. It is presentation only: **a shelf contributes no URL segment, has no page, and never appears in a breadcrumb.** Its whole job is to put a subheading above a row of cards on the page it sits in.

```text
peios.product/
└── 3--tooling.shelf/            (no URL of its own)
    ├── trail.toml
    ├── 1--pekit.topic/          →  /peios/pekit
    └── 2--provium.topic/        →  /peios/provium
```

Note the URLs: the topics live directly under the product, exactly as they would if the shelf directory were not there.

Its `trail.toml` is optional and carries at most a title:

```toml
# 3--tooling.shelf/trail.toml
title = "Tooling"
```

Without one, the title is derived from the slug — `3--tooling.shelf` becomes "Tooling" anyway, so most shelves need no config at all. A shelf may **not** declare `inline_ref`: it has no page to link to, so the phrase would have nowhere to land. Declare it on the shelf's items instead.

### How shelves render

On the parent's page, children render in order. A run of items outside any shelf renders as an untitled row of cards; each shelf renders as its title followed by its own row:

```text
Documentation

  [ Getting started ]  [ Concepts ]          ← loose items, no heading

  Tooling                                    ← shelf title
  [ pekit ]  [ provium ]

  Specifications                             ← another shelf
  [ PGSS ]  [ PCDS ]
```

Because a shelf's items share their parent's URL space, their slugs must be unique across the whole parent — two shelves cannot both contain a `getting-started` topic, since both would want `/peios/getting-started`. Trail rejects that at build time.

### The rules

- A shelf may sit in a product or an anthology.
- A shelf may contain anthologies, topics and books.
- **A shelf may never contain another shelf.** Nested headings on a card grid are not a structure worth having; if you need the second level, use an anthology.
- A shelf directory holds `trail.toml` and subdirectories only — no articles, no images.

## Choosing

Start flat. A product with four or five topics needs neither of these.

When the list gets long, ask whether the groups are worth a page. "Tooling" as a heading over two topic cards is a shelf. "Security fundamentals" — nine topics, two specifications, and a description worth reading — is an anthology.

The cost of an anthology is a URL segment on every page beneath it and one more click for the reader. The cost of a shelf is nothing at all, which is why it is usually the right first move.
