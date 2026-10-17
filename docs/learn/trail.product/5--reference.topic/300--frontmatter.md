---
title: Frontmatter Reference
type: reference
description: How to find and use the Trail interface reference for this subject.
related:
  - trail/writing/articles-and-frontmatter
  - trail/reference/trail-toml
  - trail/writing/inline-references
  - trail/reference/cli
---

<span id="description"></span>
<span id="errors"></span>
<span id="frontmatter-and-link-aliases"></span>
<span id="inline-ref"></span>
<span id="reading-minutes"></span>
<span id="related"></span>
<span id="the-keys"></span>
<span id="title"></span>
<span id="type"></span>
<span id="updated"></span>

Frontmatter gives an article its title and metadata before its Markdown
body. `trail-frontmatter(5)` is the authoritative field reference, including
types, defaults, validation, and differences between topic and book pages.

## Write useful metadata

A title helps readers choose a page. A description explains its scope in
search results and Markdown mirrors. A topic page's type distinguishes a
concept, a how-to, or a reference; books already have a document structure
and do not accept that field.

[Writing Articles and Frontmatter](~trail/writing/articles-and-frontmatter)
shows a complete example and explains how metadata affects the reading
experience. `trail build --strict` checks that descriptions are present.

## Connect related material

Related-page references guide a reader to a useful next topic. Inline
references make a recognizable phrase link to its definition. These serve
different purposes: use related pages for a reading path and inline
references for terminology.

See [Inline References](~trail/writing/inline-references) for matching and
[Writing Links and References](~trail/writing/links-and-references) for
explicit links.
