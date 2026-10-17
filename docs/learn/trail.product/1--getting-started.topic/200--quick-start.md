---
title: Quick start
type: how-to
description: Build a documentation site from an empty directory — a root trail.toml, one product, one topic, one article — then serve it with live reload and watch it grow.
related:
  - trail/getting-started/what-is-trail
  - trail/getting-started/anatomy-of-a-site
  - trail/building/build-and-serve
  - trail/writing/articles-and-frontmatter
---

This page takes you from an empty directory to a served, searchable documentation site in about five minutes. You will write four files, run `trail build`, look at what it produced, then switch to `trail serve` and add a second page while it is running.

## Prerequisites

- **A `trail` binary.** Trail is a single Rust binary; build it from its source directory with `cargo build --release` and the result is `target/release/trail`. Put it on your `PATH`, or call it by path — the examples below assume `trail` is on your `PATH`.
- **Nothing else.** The templates, stylesheet, fonts, syntax highlighter, diagram renderer and search engine are all compiled into the binary.

## Create the site

A trail site is a directory with a `trail.toml` in it. Make one:

```console
$ mkdir acme-docs && cd acme-docs
```

```toml
# acme-docs/trail.toml
sitename = "Acme Docs"
title = "Documentation for Acme"
description = "Guides and reference for the Acme toolchain."
footer = "Acme — docs."
```

Those four keys are the only required ones. `sitename` is the wordmark in the header (its last word is accented); `title` and `description` are the front page's headline and standfirst, and feed the site's `<meta>` tags. Everything else — the accent colour, the base URL, header links, a favicon — is optional and covered in [Configuring the site](~trail/building/configuring-the-site).

> [!NOTE]
> The site root may contain **only** `trail.toml`, `*.product` directories, the build output directory, and any files named by the config (a favicon, a custom stylesheet). Anything else is a build error. This is deliberate: a stray file in the root is far more likely to be a mistake than a plan.

## Add a product

Every page belongs to a **product** — a documented thing with its own landing page, colour and monogram. Products are directories named `<slug>.product`:

```console
$ mkdir -p docs.product
```

```toml
# acme-docs/docs.product/trail.toml
title = "Acme CLI"
monogram = "ac"
color = "#3b82f6"
description = "The command-line interface to Acme."
```

All four keys are required. The directory name gives the URL (`/docs`) and the slug you link by; `title` is what readers see. The `monogram` is the two-or-so characters shown on the product's tile on the front page, and `color` tints the product's cards, sidebar and headings.

## Add a topic and an article

A **topic** is a set of articles read in any order. Topics are directories named `<order>--<slug>.topic`:

```console
$ mkdir -p docs.product/1--guides.topic
```

```markdown
<!-- acme-docs/docs.product/1--guides.topic/100--hello.md -->
---
title: Hello
type: how-to
description: The first page.
---

Hello from **Acme**.
```

Three things are going on in that filename and that frontmatter block:

- **`1--guides.topic`** — the `1` sorts this topic among its siblings and never appears in a URL; `guides` is the URL segment; `.topic` is the kind. With no `trail.toml` of its own the topic's title is derived from the slug ("Guides").
- **`100--hello.md`** — same grammar for articles. Numbering in hundreds is a convention, not a rule: it leaves room to insert `150--` later without renumbering.
- **`title:` and `type:`** are the two required frontmatter keys. `description:` is optional but worth writing — it becomes the page's meta description and its social-preview text.

## Build it

```console
$ trail build
```

```text
built 6 pages (1 products) → ./dist
```

Six pages from one article, because trail writes more than the pages you authored:

```console
$ find dist -type f -not -path 'dist/assets/*' -not -path 'dist/pagefind/*'
```

```text
dist/index.html                     the front page
dist/404.html                       the not-found page
dist/docs/index.html                the product landing page
dist/docs/guides/hello/index.html   your article
dist/docs/print/index.html          all of Acme CLI on one page
dist/docs/guides/print/index.html   all of Guides on one page
dist/docs.md                        the product, as markdown
dist/docs/guides/hello.md           your article, as markdown
dist/docs/print.md                  all of Acme CLI, as one markdown file
dist/docs/guides/print.md           all of Guides, as one markdown file
dist/llms.txt                       the site's map for language models
dist/site.json                      the site's structure, machine-readable
dist/robots.txt
```

Plus `dist/assets/` (the stylesheet and fonts) and `dist/pagefind/` (the search index and its WASM core). Note that there is no page at `/docs/guides` — a topic has no page of its own. It shows up as a card on the product page, and links to it land on its first article.

`dist/` belongs to trail: at the end of every successful build it deletes anything in there that the build did not write.

## Serve it

```console
$ trail serve
```

```text
serving ./dist at http://127.0.0.1:8724 (watching . for changes)
```

`trail serve` builds the site, serves it, and watches the source tree. Save a file and the browser reloads itself; break something and the error prints in the terminal while the last good output keeps being served.

Open the site and try it: press <kbd>⌘K</kbd> (or <kbd>Ctrl K</kbd>) to search, and use the theme button in the header to switch between light, dark and system.

## Add a second page

Leave the server running and add another article beside the first:

```markdown
<!-- acme-docs/docs.product/1--guides.topic/200--installing.md -->
---
title: Installing
type: how-to
description: How to install Acme.
related:
  - docs/guides/hello
---

See [Hello](~docs/guides/hello) first.
```

Two new things:

- **`~docs/guides/hello`** is a page reference, not a URL. The first segment names a product; the rest matches the end of a page's path. Move or rename the target and the link still resolves as long as its slug is unchanged; delete it and the build fails rather than shipping a dead link. See [Links and references](~trail/writing/links-and-references).
- **`related:`** takes the same references (without the `~`) and renders a "Related content" list at the foot of the page.

Save, and the page appears in the sidebar next to Hello.

## Where to go next

- [Anatomy of a site](~trail/getting-started/anatomy-of-a-site) — the whole tree, level by level, with a real example.
- [Products](~trail/structuring-a-site/products) — the structuring section in order: products, anthologies, topics, books.
- [Articles and frontmatter](~trail/writing/articles-and-frontmatter) — every frontmatter key and what it does.
- [Configuring the site](~trail/building/configuring-the-site) — the base URL, accent colour, navigation links, edit links and the rest of `trail.toml`.
