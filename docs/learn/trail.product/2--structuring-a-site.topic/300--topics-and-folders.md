---
title: Topics and folders
type: concept
description: The topic is trail's everyday container — a set of articles read in any order, with an optional single level of subfolders. Where the sidebar comes from, why a topic has no page, and where links to one land.
related:
  - trail/structuring-a-site/products
  - trail/structuring-a-site/anthologies-and-shelves
  - trail/structuring-a-site/books
  - trail/writing/articles-and-frontmatter
---

A **topic** is a set of articles on one subject, read in any order. It is the container most pages live in, and the one to reach for unless you specifically need a [book](~trail/structuring-a-site/books).

"Getting started", "Concepts", "Configuration", "Reference" — those are topics. The reader arrives at one of the articles from search or a link, reads it, and moves on; the sidebar is there for when they want to see what else is nearby.

## The directory

A topic is a directory named `<order>--<slug>.topic`, sitting in a product, an anthology or a shelf. Its articles are the `.md` files inside it:

```text
4--reference.topic/                 (the topic; no page of its own)
├── trail.toml                      optional
├── 100--recipe-format.md           →  /pekit/reference/recipe-format
├── 200--supporting-files.md        →  /pekit/reference/supporting-files
└── 300--cli.md                     →  /pekit/reference/cli
```

The `4` orders this topic among its siblings; `reference` is the URL segment. Articles carry the same `<order>--<slug>` grammar, and their order prefixes decide reading order — the sidebar, the previous/next links, and the order they appear in the topic's single-page view.

Numbering in hundreds is the convention across this site. It is not a rule — `1--`, `2--`, `3--` works identically — but it means you can insert `150--` between two articles without renaming anything.

## The config

A topic's `trail.toml` is optional, and holds at most two keys:

```toml
# 4--reference.topic/trail.toml
title = "Reference"
```

Without it, the title is derived from the slug: `4--reference.topic` becomes "Reference", `2--writing-tests.topic` becomes "Writing Tests", `3--working-with-the-registry.topic` becomes "Working with the Registry". Write the file when that derivation is wrong — an acronym, a proper noun, unusual casing — or when the topic needs `inline_ref` phrases, which claim text that should auto-link to the topic's first article. See [Inline references](~trail/writing/inline-references).

## A topic has no page

This is the one surprising thing about topics. There is no page at `/pekit/reference` — the topic is not a destination, it is a grouping.

That has three consequences:

- **A topic appears as a card** on its product's or anthology's page, listing its articles (all of them when there are four or fewer, otherwise the first three plus a "See all" link) with a count and its total reading time.
- **Links land on the first article.** Every surface that shows a topic — its card heading, its "See all" link, the sidebar heading on an article page — points at the topic's first article in reading order.
- **A topic is not a `~link` target.** `~pekit/reference` resolves nothing, because `reference` is not the last segment of any page's path. Link to the article you mean: `~pekit/reference/cli`.

What *does* exist at the topic's path is its single-page view: `/pekit/reference/print` renders every article in the topic as one long page, and `/pekit/reference/print.md` is the same thing as markdown. See [The output surface](~trail/building/the-output-surface).

## Subfolders

A topic that has grown past a comfortable sidebar can group runs of articles into **subfolders** — plain `<order>--<slug>` directories, with no type suffix:

```text
2--concepts.topic/
├── 100--objects-and-identity.md          →  /ud/concepts/objects-and-identity
├── 200--the-schema-model.md              →  /ud/concepts/the-schema-model
└── 300--replication/                     a subfolder
    ├── trail.toml                        optional
    ├── 100--topology.md                  →  /ud/concepts/replication/topology
    └── 200--conflicts.md                 →  /ud/concepts/replication/conflicts
```

A subfolder:

- **adds a URL segment**, unlike a shelf;
- **has no page of its own**, like a topic — links from the sidebar open its first article;
- **renders as a collapsible group in the sidebar**, expanded automatically when the article being read is inside it;
- **may hold `trail.toml`, articles, `.link` files and images — but not another subfolder.** Subfolders are exactly one level deep. A topic needing more nesting than that is either two topics or a book.

Its optional `trail.toml` takes the same `title` and `inline_ref` keys a topic's does, and its title derives from its slug the same way.

An empty subfolder is tolerated and simply not shown, so you can create the directory before its articles exist.

## The sidebar

On an article page, the sidebar shows the article's own topic: the topic title (linking to its first article) above the topic's contents in reading order, with subfolders as collapsible groups and the current article marked. It shows one topic — not the whole product — because the topic is the unit a reader is working within.

Below it, "On this page" lists the article's own `h2` and `h3` headings, and highlights the one currently in view.

## Reading order and the pager

The prev/next links at the foot of an article step through the whole topic in reading order, subfolder contents flattened in place. The last article of one topic does not lead to the first of the next; the pager is a topic-local device.

## Topic or book?

| Use a topic when | Use a book when |
|---|---|
| Articles can be read in any order | The document is read in order, or cited by section |
| No page needs to say "§2.4" | Sections need stable numbers |
| Grouping is for navigation | The whole thing is one document with a cover |
| One level of subfolders is enough | Nesting goes deeper than two levels |

Almost all documentation is topics. Books are for specifications, formal references and manuals — see [Books](~trail/structuring-a-site/books).
