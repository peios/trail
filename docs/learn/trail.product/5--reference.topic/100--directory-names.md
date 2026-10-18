---
title: Directory and File Names
type: reference
description: How to find and use the Trail interface reference for this subject.
related:
  - trail/getting-started/anatomy-of-a-site
  - trail/reference/trail-toml
  - trail/reference/frontmatter
  - trail/structuring-a-site/books
---

<span id="a-n-appendix-orders"></span>
<span id="duplicates"></span>
<span id="images"></span>
<span id="kind"></span>
<span id="order"></span>
<span id="slug"></span>
<span id="sorting"></span>
<span id="the-name-shapes"></span>
<span id="trail-toml-by-level"></span>
<span id="urls"></span>
<span id="what-each-directory-may-contain"></span>

Trail uses directory names to derive navigation and URLs. A source name
contains an order, a slug, and sometimes a kind suffix. `trail-source(5)`
is the authoritative naming and containment reference.

## Read a source name

In `200--getting-started.topic`, `200` controls sibling order,
`getting-started` supplies the URL slug, and `.topic` selects the grouping
kind. Root collections use a slug and `.product` or `.antho`, without an
order.

Choose topics for independently readable pages and books for an ordered
document with section numbers. Shelves group navigation without adding a
URL segment. See [Anatomy of a Site](~trail/getting-started/anatomy-of-a-site)
for a complete example tree.

## Keep names stable

Renaming a slug changes URLs and references. Order changes usually affect
navigation only; in books they also affect published section numbers.
Run `trail build --strict` after structural changes to find broken links.

Use [link aliases](~trail/structuring-a-site/link-aliases) when the same page
belongs in several navigation locations. An alias does not duplicate the
source article.

## Reserved names

Trail publishes generated pages and asset directories alongside authored
content. Some slugs are reserved to prevent collisions. `trail-source(5)`
lists them and the allowed files at each level of a tree.
