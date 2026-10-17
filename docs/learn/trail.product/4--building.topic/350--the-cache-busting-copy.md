---
title: The cache-busting copy
type: concept
description: What --cbpath publishes, why a second copy of the whole site is the only version of the idea that works, what carries the path prefix and what deliberately does not, and how to wire it into a deploy.
related:
  - trail/building/the-output-surface
  - trail/building/the-reading-experience
  - trail/building/build-and-serve
  - trail/reference/cli
---

Static hosts serve your pages with cache headers you do not control. GitHub Pages sets its own; so does every CDN in front of it. For the minutes or hours those headers claim, a reader — or, far more often, an automated fetcher — can be served the *previous* build with nothing to tell them apart.

`--cbpath` answers that by publishing a second, complete copy of the site under a path you name:

```sh
trail build --cbpath 8f3a91c
```

```text
built 1227 pages (8 products) → ./dist
cache-busting copy → ./dist/8f3a91c/
```

Now `/8f3a91c/` is the site's front page and `/8f3a91c/pekit/reference/cli` is that article. Name the copy after something that changes on every deploy — a commit hash — and every URL inside it has never been requested by anyone, so nothing anywhere can have it cached. What you get back is necessarily this build.

## Why a copy and not a redirect

The obvious cheap version — one busted URL that redirects, or a byte-for-byte copy of the tree — does not work, and fails in a way you cannot see.

Every URL trail emits is site-absolute: `/pekit/reference/cli`, `/assets/style.css`, `/pagefind/pagefind.js`. A plain copy at `/8f3a91c/` would serve you a fresh HTML document for the one page you asked for, and that document would then load the **cached** stylesheet, the **cached** search script, and every link out of it would land back in the cached tree. One click and you are reading the old build again, with nothing on the page to say so.

So the copy is not copied. It is built, with the path baked into every URL it emits.

## What carries the prefix

| | |
|---|---|
| Every link between pages | breadcrumbs, sidebars, cards, the pager, `~` references in prose |
| `§` section citations and [inline references](~trail/writing/inline-references) | |
| Images that live in the tree | the ones trail publishes for you |
| Assets | the stylesheet, the search script, mermaid, your `custom.css`, the favicon |
| The search index | so a result inside the copy stays inside it |
| The whole [markdown surface](~trail/building/the-output-surface) | `.md` mirrors, `print.md`, `llms.txt`, `site.json` — the copy is mostly *for* machine readers, so these matter most |
| `passthrough` entries | copied into the copy as well as the site |

And what does not:

| | |
|---|---|
| `<link rel="canonical">` | names the **real** page, so the copy never competes with it |
| URLs you wrote out in full | `https://…`, and a leading-slash path you typed yourself — trail emits those exactly as written rather than second-guessing them |
| `edit_url` | it points at your repository, not at the site |
| `robots.txt` and `sitemap.xml` | see below |

## Search engines

The copy is a complete duplicate of the site at a path that changes on every deploy — exactly the thing a search engine should never index. Trail makes sure it isn't:

- every page in the copy carries `<meta name="robots" content="noindex, follow">`;
- every page names the real page as its canonical;
- the copy has no `sitemap.xml`, and the site's sitemap does not mention it.

There is deliberately **no** `Disallow` in `robots.txt`. Some automated fetchers honour `robots.txt` on a URL you handed them directly, and blocking the copy would defeat the point of having it.

## The path

One plain segment, made of letters, digits, `-`, `_` and `.`, not starting with a dot. It must not be a path the site already uses — a product slug, `assets`, `pagefind`, `site.json`, a `passthrough` entry — or the copy would bury it. Trail checks before it builds anything:

```text
Error: --cbpath 'assets' is already a path in the site; the copy would bury it — pick a name the site does not use
```

## What it costs

The output roughly doubles: twice the pages, twice the mirrors, a second search index. On a large site that is real — Peios Learn goes from 115 MB to 232 MB, and its deploy artifact from 27 MB to 54 MB. Build time roughly doubles too. Nothing about the site itself gets slower; the copy is only ever fetched by someone who asks for it by name.

## In a deploy

The point is to inject the commit, so the path is new on every push:

```yaml
- name: Build site
  run: trail build --cbpath "${GITHUB_SHA::8}"
```

On a `push`, `GITHUB_SHA` is the commit that `actions/checkout` just checked out, so the path and the content it names cannot disagree. The first eight characters are plenty to be unique per deploy and short enough to paste by hand.

Each deploy replaces the output wholesale, so only the current commit's copy exists at any moment — the previous one is pruned along with everything else the build did not write. Do not bookmark one.

## Finding it as a reader

The site itself learns where its copy is, and the search box gets a command for it. Type `/cb` (or `/cachebust`) and you land on the same page you were reading, in the copy. Type it again from inside the copy and you come back out. See [the reading experience](~trail/building/the-reading-experience).
