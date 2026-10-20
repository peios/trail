---
title: trail.toml Reference
type: reference
description: How to find and use the Trail interface reference for this subject.
related:
  - trail/building/configuring-the-site
  - trail/reference/directory-names
  - trail/reference/frontmatter
  - trail/building/theming
---

<span id="a-book"></span>
<span id="a-product"></span>
<span id="a-shelf"></span>
<span id="a-topic-subfolder-or-chapter"></span>
<span id="an-anthology"></span>
<span id="inline-ref-everywhere"></span>
<span id="merge"></span>
<span id="nav"></span>
<span id="passthrough"></span>
<span id="attachments"></span>
<span id="plugins-name"></span>
<span id="the-site-root"></span>
<span id="validation"></span>
<span id="what-url-unlocks"></span>

Each `trail.toml` configures the source directory that contains it. The
root file defines site-wide presentation, merged trees, attachments, plugins,
and writing rules. Grouping files describe their own navigation unit.
`trail.toml(5)` is the authoritative key and validation reference.

## Configure the site root

Start with [Configuring the Site](~trail/building/configuring-the-site).
Set a public URL when you publish the site so canonical links and discovery
files can use absolute addresses. Keep deployment-specific values in the
root configuration rather than repeating them in articles.

## Configure a grouping

Products, anthologies, and books carry their own presentation metadata.
Topics, chapters, and shelves can override their derived titles. The
accepted keys differ by kind; `trail.toml(5)` lists each set.

## Derived titles

When a grouping has no explicit title, Trail converts its slug to title
case. The first and last words are capitalized; internal articles,
conjunctions, and short prepositions generally stay lowercase.
Use an explicit title for a proper name or an identifier whose casing
cannot be derived from the slug.

## Reuse terminology

An inline reference associates a phrase with a page or grouping. The phrase
then links to that destination when used in prose. See
[Inline References](~trail/writing/inline-references) for examples.

## Add generated content

A configured plugin expands named Markdown fences. Its command runs with
the privileges of the build process, so review plugin configuration before
building a source tree. `trail-plugin(5)` documents the process interface.
[Writing Plugin Blocks](~trail/writing/plugin-blocks) explains how to use it.

## Check writing

The optional `[lint]` table enables writing checks. `trail-lint(5)` documents
the rules and configuration. [Checking Style](~trail/writing/checking-style)
walks through running the check and interpreting findings.
