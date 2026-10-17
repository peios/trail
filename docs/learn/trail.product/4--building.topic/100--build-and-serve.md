---
title: Building and serving
type: how-to
description: The two commands — trail build writes a static site into dist/, trail serve adds a watching dev server with live reload. What each flag does, what gets pruned, and how failures behave.
related:
  - trail/reference/cli
  - trail/building/the-output-surface
  - trail/writing/links-and-references
  - trail/getting-started/quick-start
---

Trail has two commands. `build` writes the site; `serve` writes it and then serves it, rebuilding as you edit.

## trail build

```console
$ trail build [ROOT] [--out DIR] [--strict] [--allow-dangling-links] [--render-llms-full] [--cbpath PATH]
```

`ROOT` is the directory containing `trail.toml`, defaulting to the current directory. Output goes to `ROOT/dist` unless `--out` says otherwise.

```console
$ trail build
built 1227 pages (8 products) → ./dist
```

The page count is HTML pages: articles, product and anthology landings, book covers, `/print` bundles, the front page and the 404 page. The markdown mirrors, `llms.txt`, `site.json`, the sitemap and the search index are not counted as pages.

### The output directory belongs to trail

At the end of every **successful** build, trail deletes everything in the output directory it did not write on that run, then removes any directory left empty. Pages for articles you deleted, superseded search-index fragments, images you stopped referencing: all gone.

Two consequences:

- **Do not keep anything in `dist/` by hand.** A `CNAME`, a `_headers` file, a stray favicon — trail will delete it. Anything that must ship belongs in the source tree and named by [`passthrough`](~trail/building/configuring-the-site), which copies it in on every build.
- **A failed build prunes nothing.** The previous output is left completely intact, which is what makes the dev server able to keep serving a working site while you fix an error.

Files whose contents have not changed are left untouched rather than rewritten, so modification times stay stable for rsync-style deploys.

### When a build fails

Loading errors — a bad name, an unknown config key, a malformed frontmatter block — stop the build at the first problem, with the context chained on:

```text
Error: loading product 'pekit'

Caused by:
    0: loading topic in '2--recipes.topic'
    1: loading article 'anatomy'
    2: ./pekit.product/2--recipes.topic/100--anatomy.md has unterminated frontmatter
```

Link errors are different: they are **collected across the whole site** and reported together, because fixing them one build at a time would be miserable.

```text
Error: 3 broken links:
in article '/pekit/recipes/anatomy': link '~pekit/running/invokation' matches no page in product 'pekit'
…
```

Some problems are warnings on stderr rather than errors, and do not stop the build: an image nothing references, and (with `--allow-dangling-links`) missing link targets.

## The flags

### `--allow-dangling-links`

Downgrades missing `~link` and `related:` targets from errors to warnings, and does the same for missing `#fragment` anchors, relative-path destinations, missing images and unresolvable `§` citations. **Ambiguous references stay fatal** — trail will not pick between two candidate pages.

It is for the middle of a large reorganisation, when you want to see the site before every reference has caught up. Shipping with it on means shipping dead links.

### `--strict`

Fails the build when any article has no `description:`:

```text
Error: --strict: 2 articles without a description:
  article '/pekit/recipes/sources' has no description
  article '/pekit/reference/cli' has no description
```

Descriptions become meta descriptions and social-preview text, so a page without one is a page that presents badly everywhere outside the site. Alias pages are exempt — the original carries the description.

Use it in CI; leave it off while drafting.

### `--render-llms-full`

Writes an extra copy of each unit's `print.md` as `llms-full.txt` beside it. Purely a discovery fallback for agents that probe for that filename instead of reading `llms.txt`, which always points at `print.md` anyway. Off by default, because it doubles a large part of the output for no new information.

### `--cbpath PATH`

Publishes a second copy of the whole site under `PATH`, with every link inside it rewritten to stay inside it. Name it after something that changes on every deploy — a commit hash — and you have a URL nothing can have cached, so what comes back is necessarily this build. See [The cache-busting copy](~trail/building/the-cache-busting-copy).

### `--out DIR`

Writes the site somewhere other than `ROOT/dist`. The chosen directory is excluded from the site scan, so a build into a directory inside the site root does not turn the previous build into content.

## trail serve

```console
$ trail serve [ROOT] [--addr ADDR] [build flags]
```

`serve` takes every `build` flag, plus `--addr` (default `127.0.0.1:8724`).

```console
$ trail serve
serving ./dist at http://127.0.0.1:8724 (watching . for changes)
```

It builds the site, serves the output directory over HTTP, and watches the source tree. On a change it debounces for 200 ms of quiet — editors write in bursts — rebuilds, and tells every open page to reload itself over a server-sent-events stream. Changes inside the output directory are ignored, so the build cannot trigger itself, as are paths with a dot-prefixed component, so a `.git` write does not either.

Files that [plugins](~trail/writing/plugin-blocks) report depending on are watched too, even outside the source tree. A plugin that reads `../drivers/base/core.c` rebuilds the site when that file changes, but not when its neighbours do. A directory a plugin depends on is watched with everything beneath it.

[Merged trees](~trail/structuring-a-site/merged-trees) are watched like the site root, except for each tree's own `dist/`, and the startup line lists them after the root.

```text
rebuilt 1227 pages
```

**A broken site does not kill the server.** A failed initial build or rebuild prints the error and keeps serving the last good output:

```text
rebuild failed (still serving the last good build): loading product 'pekit': …
```

Fix the file and the next successful rebuild reloads the page.

Two details worth knowing:

- **The live-reload script is only ever injected by `serve`.** Output written by `trail build` never contains it.
- **Misses serve the built `404.html` with a real 404 status**, exactly as a static host configured for that file would — so you can check the not-found page locally.

## Deploying

The output directory is plain static files. Any host that serves a directory will do.

Two things to configure on the host:

- **`404.html` as the not-found page.** Trail writes it; the host has to be told to use it.
- **Directory indexes.** Pages are written as `<path>/index.html`, so the URL `/pekit/reference/cli` needs to serve `/pekit/reference/cli/index.html`. Nearly every static host does this by default.

Set `url` in the root `trail.toml` before deploying: without it there is no `sitemap.xml`, no `Sitemap:` line in `robots.txt`, and canonical and `og:url` tags stay relative. See [Configuring the site](~trail/building/configuring-the-site).

If the host's caching is aggressive enough that you cannot tell a fresh deploy from a stale one — and on GitHub Pages you cannot, because the headers are not yours to set — build with `--cbpath "${GITHUB_SHA::8}"` and you get a URL per deploy that nothing has ever cached. See [The cache-busting copy](~trail/building/the-cache-busting-copy).
