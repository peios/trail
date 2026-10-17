---
title: The output surface
type: reference
description: Everything a build writes into dist/ — HTML pages, single-page /print views, markdown mirrors of every page, llms.txt, site.json, sitemap.xml, robots.txt, the 404 page, and the search bundle.
related:
  - trail/building/build-and-serve
  - trail/writing/links-and-references
  - trail/building/the-reading-experience
  - trail/reference/cli
---

A build writes rather more than the pages you authored. This page is the full inventory of what lands in the output directory and what each part is for.

```text
dist/
├── index.html                        the front page
├── 404.html                          the not-found page
├── robots.txt
├── sitemap.xml                       only when the site config has a `url`
├── llms.txt                          the site's map, for language models
├── site.json                         the site's structure, machine-readable
├── pekit.md                          a product, as markdown
├── pekit/
│   ├── index.html                    the product landing page
│   ├── print/index.html              all of pekit on one page
│   ├── print.md                      all of pekit as one markdown file
│   └── reference/
│       ├── cli/index.html            an article
│       ├── cli.md                    the same article, as markdown
│       ├── print/index.html          all of the Reference topic on one page
│       └── print.md
├── assets/                           style.css, search.js, fonts, licences
└── pagefind/                         the search index and its WASM core
```

## HTML pages

One page per article, product, anthology and book cover, plus the front page, the 404 page and one `/print` page per unit. Each is written as `<path>/index.html`, so `/pekit/reference/cli` needs a host that serves directory indexes — nearly all of them do.

Topics, subfolders, chapters and shelves have no page. The count `trail build` prints is exactly this set.

## Single-page views: /print

Every **unit** — a product, an anthology, a topic or a book — also publishes its whole contents as one page:

```text
/pekit/print                  every article in pekit, in reading order
/pekit/reference/print        every article in the Reference topic
/peios/pgss/print             the whole book
```

Each article becomes a section with its title, its breadcrumb, and a heading id unique within the bundle. The page is styled for reading and for printing: browser chrome, the site header and footer, the pager and the tab buttons all drop out when it goes to paper, and tab panels expand so nothing is hidden.

Links are rewritten so the document works away from the site: a link to a page **inside** the same bundle becomes an in-document anchor, and one **outside** it becomes an absolute URL when the site config has a `url`, staying root-relative otherwise.

Every article page carries a "Single page" link in its breadcrumbs, pointing at its unit's bundle **anchored at that article** — so a reader can go from a page to the whole document without losing their place.

Two things do not carry over into a bundle: [inline reference phrases and `§` citations](~trail/writing/inline-references) are not linked there. RFC-2119 keywords still are.

A unit with no articles gets no bundle.

## Markdown mirrors

**Every page is also available as markdown at its own URL with `.md` appended.**

```text
/pekit/reference/cli      →  /pekit/reference/cli.md
/pekit                    →  /pekit.md
/pekit/reference/print    →  /pekit/reference/print.md
```

A mirror is the article's *source*, not a re-derivation of the HTML — the same headings, the same fenced code blocks with their info strings, the same tables — with three rewrites:

- `~` references become the target's own `.md` URL, so following links keeps you in the markdown surface;
- relative image destinations become the images' published URLs, since the mirror is served away from the article's folder;
- inside a book, `h2`/`h3` headings gain their section numbers as text.

[Plugin blocks](~trail/writing/plugin-blocks) are the one place a mirror departs from what was typed: it shows the plugin's expanded output, not the fence that asked for it.

Each one opens with the title, the breadcrumb trail, and the description:

```markdown
# Installing

_Acme CLI / Guides_

> How to install Acme.

See [Hello](/docs/guides/hello.md) first.

Related content:

- [Hello](/docs/guides/hello.md)
```

Every HTML page also advertises its mirror in the head:

```html
<link rel="alternate" type="text/markdown" href="/pekit/reference/cli.md">
```

## llms.txt

A map of the site at the root, following the [llms.txt](https://llmstxt.org) convention: the sitename and description, a note that every page has a `.md` twin, and then one section per product listing its units by their `print.md` bundles.

```text
# Peios Learn

> Concepts, guides, and reference for the Peios operating system…

Every page on this site is also available as markdown at the same URL with
`.md` appended. Each section below links whole units as single markdown
files. The full machine-readable structure is at /site.json.

## pekit

- [pekit overview](/pekit.md): The recipe-driven build and packaging tool…
- [All pekit docs in one file](/pekit/print.md)
- [Getting started](/pekit/getting-started/print.md): 2 articles
```

The `--render-llms-full` flag additionally writes each unit's `print.md` as `llms-full.txt` beside it, for agents that probe for that filename rather than reading `llms.txt`. It is off by default: it duplicates a large part of the output and adds nothing new.

## site.json

The whole site model as JSON: the site's own metadata, then products, and inside them the full tree of anthologies, topics, folders, books, chapters and articles.

```json
{
  "sitename": "Peios Learn",
  "url": "https://learn.peios.org",
  "llms": "/llms.txt",
  "products": [
    {
      "slug": "pekit",
      "path": "/pekit",
      "md": "/pekit.md",
      "print": "/pekit/print.md",
      "title": "pekit",
      "monogram": "pk",
      "color": "#22c55e",
      "description": "…",
      "items": [
        {
          "kind": "topic",
          "slug": "reference",
          "path": "/pekit/reference",
          "print": "/pekit/reference/print.md",
          "title": "Reference",
          "entry": "/pekit/reference/recipe-format",
          "children": [
            {
              "slug": "cli",
              "path": "/pekit/reference/cli",
              "md": "/pekit/reference/cli.md",
              "title": "Command-line reference",
              "type": "reference",
              "description": "…",
              "related": ["/pekit/running/invocation"]
            }
          ]
        }
      ]
    }
  ]
}
```

Every node carries the URLs of its own alternative representations, so a consumer can walk the structure and fetch exactly the parts it wants. Book entries add `number` and, for appendices, `appendix: true`; [alias pages](~trail/structuring-a-site/link-aliases) add `original`.

## sitemap.xml and robots.txt

`robots.txt` is always written, allowing everything, with a `Sitemap:` line when the site config has a `url`.

`sitemap.xml` is written **only** when it has one, because sitemaps require absolute URLs. It lists every HTML page — the front page, products, anthologies, book covers, articles and `/print` bundles — each with a `<lastmod>` taken from that page's `updated:` date, or for a container, from the most recent one beneath it. [Alias pages](~trail/structuring-a-site/link-aliases) are left out: their canonical is the original.

## The 404 page

`404.html` is a real page in the site's theme, carrying `<meta name="robots" content="noindex">`. Trail writes it; your host has to be told to use it. `trail serve` already does, and returns a real 404 status with it.

## Search

`/pagefind/` holds a [Pagefind](https://pagefind.app) index built from the rendered pages: the index chunks, the WASM search core, and the `pagefind.js` module the theme's search script imports.

The front page is not indexed. Neither are [alias pages](~trail/structuring-a-site/link-aliases) — the original covers the same content — nor the parts of a page marked as chrome: breadcrumbs, the meta line, the Related Content list, the pager and the edit link. Each indexed page carries its breadcrumb trail as metadata, which is what search results show under the title.

## Assets

`/assets/` holds the stylesheet, the search script, the two variable fonts with their licences, and `LICENSE-Syntaxes.txt` — the licences of the bundled syntax-highlighting definitions, which ask to travel with them. `mermaid.min.js` and its licence join them only if some article on the site contains a [diagram](~trail/writing/admonitions-tabs-and-diagrams). A configured `custom_css` ships as `/assets/custom.css`; a configured `favicon` ships at the output root under its own name.

Images are published at the URL of the directory they were found in — but only the ones some article actually references. See [Images](~trail/writing/images).

## The cache-busting copy

`--cbpath <PATH>` adds one more entry to the output: `dist/<PATH>/`, holding everything above a second time, with every link inside it rewritten to stay inside it. It is how you get a URL that no cache can have seen. See [The cache-busting copy](~trail/building/the-cache-busting-copy).

## Passthrough files

Anything named by the root config's [`passthrough`](~trail/building/configuring-the-site) is copied in verbatim — a `CNAME`, a `.well-known/` directory, a verification file — keeping its path relative to the site root. It is written after everything else, so a passthrough named for a generated file (`robots.txt`) replaces it.

Passthrough files are tracked like any other output, so they survive the prune between builds.
