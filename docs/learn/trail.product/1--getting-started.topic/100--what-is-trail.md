---
title: What is Trail
type: concept
description: Trail is the static site builder behind Peios Learn — a single Rust binary that turns a directory tree of markdown into a searchable, themed documentation site, with the structure coming from the filesystem rather than a config file.
related:
  - trail/getting-started/quick-start
  - trail/getting-started/anatomy-of-a-site
  - trail/structuring-a-site/products
  - trail/reference/cli
---

**Trail** is a static site generator for documentation. You give it a directory of markdown files arranged into named folders; it gives you a complete website — every page rendered, cross-linked, indexed for search, mirrored as markdown for machines, and bundled into printable single-page views.

This site is built with it. Everything you are reading, including this page, comes out of one `trail build`.

## The one idea: the tree is the site

Trail has no table-of-contents file, no `sidebar.json`, no list of pages to keep in sync with the pages themselves. **The directory tree is the navigation.** A folder named `2--writing.topic` is a topic called "Writing" that sorts second; the `.md` files inside it are its articles, in the order their own numeric prefixes give them.

```text
learn/
├── trail.toml                        # the site
└── trail.product/                    # a product
    ├── trail.toml
    ├── 1--getting-started.topic/     # a topic, sorted first
    │   ├── 100--what-is-trail.md     # an article, sorted first
    │   └── 200--quick-start.md
    └── 2--structuring-a-site.topic/
        └── 100--products.md
```

Add a file, and the page exists, appears in the sidebar, joins the search index, and gets a markdown mirror. Rename a folder, and the section moves. Nothing else has to be told.

The trade is that names carry meaning. `100--quick-start.md` is not a decorative filename: `100` is the sort key and `quick-start` is the URL segment. [Directory and file names](~trail/reference/directory-names) is the exhaustive grammar; [Anatomy of a site](~trail/getting-started/anatomy-of-a-site) is the guided tour.

## Strict by default

The second principle is that **mistakes are build failures, not silent surprises**. A documentation site that quietly drops a page is worse than one that refuses to build.

- A key that `trail.toml` does not define is an error, not an ignored line.
- A file trail does not recognise — a stray `notes.txt` in a topic, a `README.md` without an order prefix — is an error, not a file left out of the site.
- A `~link` whose target no longer exists fails the build. Every broken link in the site is reported at once, so one run tells you the whole story.
- An ambiguous `~link` — one matching two pages — always fails, even under `--allow-dangling-links`. Trail will not guess which page you meant.
- A frontmatter `updated:` that is not an ISO date, a colour that is not `#rrggbb`, a `.link` alias pointing at another alias: all errors.

The escape hatches are deliberate and few. `--allow-dangling-links` downgrades missing targets to warnings while you are mid-refactor; that is the whole list.

## One binary, no runtime

Trail is a single Rust binary with everything compiled in: the templates, the stylesheet, the fonts and their licences, the search engine, the syntax highlighter, the diagram renderer. There is no theme directory to install beside it, no Node toolchain, no `npm install` step, and nothing to fetch at build time.

The output is equally plain. `dist/` is a directory of static files — HTML, CSS, WOFF2, JSON, a WASM search core — that any static host will serve as-is. There is no server component and no build step at request time.

## What one build does

```text
trail.toml + the tree
  → load          parse every trail.toml and every frontmatter block, validate names
  → resolve       .link aliases, ~link targets, inline-reference phrases, images
  → render        one HTML page per article, product, anthology and book cover
  → export        .md mirrors, /print bundles, llms.txt, site.json, sitemap, robots
  → index         the Pagefind search bundle, built from the rendered pages
  → prune         delete anything in dist/ this build did not write
```

Loading is where nearly all the errors live: by the time rendering starts, the site model is known to be well-formed. Pruning is why `dist/` belongs entirely to trail — a page you deleted from the source disappears from the output on the next successful build, along with superseded search index fragments and stale images.

Builds are fast enough not to think about. This site — over twelve hundred pages — builds from scratch in a little over three seconds.

## What a site is made of

Four kinds of thing, from the outside in:

| | What it is |
|---|---|
| **Site** | One `trail.toml` at the root, the front page, and the products. |
| **Product** | A documented thing (`trail.product/`) with its own colour, monogram and landing page. |
| **Grouping** | How a product's pages are organised: [anthologies](~trail/structuring-a-site/anthologies-and-shelves), [topics](~trail/structuring-a-site/topics-and-folders), [books](~trail/structuring-a-site/books) and shelves. |
| **Article** | One `.md` file: YAML frontmatter, then markdown. |

The distinction that matters most is **topic versus book**. A topic is a set of articles you can read in any order — a how-to section, a concept section. A book is a formal, ordered document — a specification, a reference manual — with a cover page, dotted section numbers (`2.1.3`), chapters that nest arbitrarily deep, and lettered appendices. They render differently because they are read differently.

## What trail gives readers

Beyond the pages themselves, every build produces:

- **Search** — a ⌘K modal over a [Pagefind](https://pagefind.app) index, loaded lazily so pages carry no search cost until someone searches. Results carry the query through to the page, which highlights every match.
- **Three-state theming** — light, dark, or follow the system, remembered per reader. Syntax highlighting, diagrams and images all follow it.
- **Print and machine views** — every topic, book, anthology and product is also one long page at `/print`, one markdown file at `/print.md`, and every individual page is mirrored at its own URL with `.md` appended.
- **The reading furniture** — a scroll-spying "On this page" list, copy buttons on code blocks, heading permalinks, adjustable text size, previous/next links, a mobile drawer, and a back-to-top button.

[The reading experience](~trail/building/the-reading-experience) covers all of it; [The output surface](~trail/building/the-output-surface) covers exactly what lands in `dist/`.

## What trail is not

- **Not a general-purpose SSG.** There is no plugin system, no shortcode API, no way to add a page type. The templates are compiled in and are not user-replaceable; [theming](~trail/building/theming) happens through CSS custom properties and an optional stylesheet.
- **Not versioned.** One build produces one version of the site. Hosting several versions means several builds at several paths.
- **Not incremental.** Every build renders every page. It is fast enough that this has never mattered.
- **Not a subpath host.** Sites are served from the root of a domain; internal links are root-relative.

## Where to start

If you want a working site in five minutes, go to the [Quick start](~trail/getting-started/quick-start).

If you want to understand the layout before writing anything, read [Anatomy of a site](~trail/getting-started/anatomy-of-a-site), then the [Structuring a site](~trail/structuring-a-site/products) section in order.

If you are writing pages in a site that already exists, [Writing](~trail/writing/articles-and-frontmatter) is the section you want — frontmatter, links, images, code blocks and the rest.
