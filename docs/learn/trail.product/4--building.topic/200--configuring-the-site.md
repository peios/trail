---
title: Configuring the Site
type: how-to
description: The root trail.toml — the four required keys, the base URL and what it unlocks, featured products, header navigation, favicon, edit links, and injecting your own HTML into every head.
related:
  - trail/reference/trail-toml
  - trail/building/theming
  - trail/building/build-and-serve
  - trail/structuring-a-site/products
---

<span id="what-may-live-in-the-site-root"></span>

One `trail.toml` at the site root configures the whole site. It is the only
place these settings live — there is no per-environment config, no CLI
equivalent for most of them, and unknown keys are build errors rather than
typos you find out about later.

```toml
sitename = "Peios Learn"
title = "Documentation for everything Peios"
url = "https://learn.peios.org"
accent = "#3b82f6"
description = "Concepts, guides, and reference for the Peios operating system and every project around it — from the kernel to the toolchain that builds it."

featured = ["peios", "pekit", "provium", "universal-directory", "trail"]

footer = "Peios Learn — documentation for the Peios project."
```

`trail.toml(5)` is the authoritative key reference; this page is what each
setting is _for_.

## Complete the task

1. Open the existing site root's `trail.toml`, or create it with the required
   values shown below.

2. Set the presentation and publishing values needed for the site.
   `trail.toml(5)` describes accepted keys and defaults.

3. Run `trail build --strict` from that directory and resolve configuration
   errors.

4. Preview the output and check the site name, navigation, footer, and public
   URLs.

## The four required keys

`sitename`, `title`, `description`, and `footer` must all be present.

**`sitename`** is the site's identity: the wordmark in the header, the suffix
on every page's `<title>`, the search box's placeholder, and `og:site_name`.
Its **last word is accented** in the wordmark — "Peios _Learn_" — so a
two-word name reads best. A one-word name gets no accent.

**`title`** and **`description`** are the front page's headline and
standfirst, and its meta description and social-preview text. The description
is also the first line of `llms.txt`, so write it as a summary of the whole
site rather than a slogan.

**`footer`** is the line at the foot of every page. Beneath it, "Built with
Trail." appears unless `built_by_trail = false` turns it off.

## The base URL

```toml
url = "https://learn.peios.org"
```

Optional, and worth setting before you deploy. Without it:

- **no `sitemap.xml` is written**, because sitemaps require absolute URLs, and
  `robots.txt` gets no `Sitemap:` line;
- **canonical URLs stay root-relative** rather than absolute;
- **`og:url` is omitted**, since a relative one is meaningless to the crawlers
  reading it;
- **`/print` bundles link outward relatively**, which is right on the site and
  wrong on paper.

Trailing slashes are trimmed, so `https://learn.peios.org/` and
`https://learn.peios.org` behave identically.

> [!NOTE]
> Trail builds for the **root of a domain**. Internal links are
> root-relative (`/pekit/reference/cli`), so serving the site from a subpath
> is not supported.

## Featured products

```toml
featured = ["peios", "pekit", "provium", "universal-directory", "trail"]
```

The list does two jobs: it orders products, and it decides which appear on the
front page.

- Products in the list sort first, in the list's order; everything else
  follows, by title.
- **Only listed products get a card on the front page.** An unlisted product
  is still built, still searchable, still linkable, and still in the header's
  documentation menu.
- Naming a product that does not exist is a build error.

## Header navigation

```toml
[[nav]]
label = "Specs"
url = "~peios/pgss"

[[nav]]
label = "Source"
url = "https://github.com/peios/peios"
```

Each `[[nav]]` block adds a link beside the documentation menu. On
narrow screens they move inside that menu.

Keep the `[[nav]]` blocks at the **end of the file**: in TOML every key after
a table header belongs to that table, so an ordinary key written below them
becomes a nav field and fails the build.

`url` can be an external URL, a root-relative path, or a
[`~` page reference](~trail/writing/links-and-references). References are
resolved **strictly, at load time** — even with `--allow-dangling-links`, a
broken nav link fails the build. Site chrome appears on every page; it must
never dangle.

## Favicon

```toml
favicon = "favicon.png"
```

A path relative to the site root. The file is copied to the output root under
its own name and linked from every page. The file must exist, or the build
fails.

Naming a file here is also what makes it _legal_ to keep in the site root,
which otherwise accepts `trail.toml`, `*.product`, and `*.antho` directories.

## Edit links

```toml
edit_url = "https://github.com/peios/learn/edit/main/{path}"
```

Adds an "Edit this page" link to the foot of every article. `{path}` is
replaced with the article's source file path relative to the site root — the
placeholder is required, and a template without it is a build error.

For an [alias page](~trail/structuring-a-site/link-aliases) the link points at
the _original's_ source file, since that is the file to edit.

Pages from a [merged tree](~trail/structuring-a-site/merged-trees) use that
tree's own `edit_url`, with `{path}` relative to that tree's root, because the
tree usually lives in a different repository. A merged tree without `edit_url`
gets no edit links.

## Injecting your own head HTML

```toml
head_html = "head.html"
```

A path to a file whose contents are injected verbatim at the end of every
page's `<head>`: analytics snippets, extra meta tags, a webfont, a
verification token.

It is inserted unescaped and unchecked, at the end of the head, so it can
override earlier tags. It is the escape hatch for everything trail does not
have a setting for — and, like all escape hatches, the thing to reach for
last.

## Files trail does not build

For downloadable files or source bundles, use
[Attachments](~trail/writing/attachments). Attachment declarations provide
checked references, automatic packaging, and size and checksum metadata.
Use `passthrough` for files that need an unchanged public path.

```toml
passthrough = ["CNAME", ".well-known"]
```

Some files have to be in the published site without trail having any opinion
about them: a `CNAME` for a custom domain, a `.well-known/` directory, a
search-engine verification file. `passthrough` names them, and each one is
copied into the output exactly as it is — directories whole.

This matters most when the site root **is** a repository root. Trail accepts
root collections and `trail.toml` there, so a `CNAME` sitting
beside them would otherwise fail the build. Naming it here makes it legal
_and_ publishes it.

Two details worth knowing:

- Entries are copied **after** everything trail generates, so naming
  `robots.txt` replaces the generated one — useful, and a foot-gun if you name
  `assets`.
- Dot-entries like `.github/` are already skipped by the root scan, so they
  only need naming here if you want them **published**. `.well-known/` does;
  `.github/` does not.

> [!TIP]
> A repository whose root is a trail site cannot hold a `README.md` —
> it is an unexpected root file. Put it at `.github/README.md`, which GitHub
> renders on the repository page and trail skips.

## Theming

Four keys control color:

```toml
accent = "#3b82f6"
accent_dark = "#7aa8ff"     # optional; derived from accent if absent
custom_css = "custom.css"   # optional
product_theming = true      # default
```

`accent` is the site's color. `accent_dark` overrides its dark-mode variant,
which is otherwise derived by mixing the accent toward white. `custom_css`
names a stylesheet shipped at `/assets/custom.css` and loaded after the
built-in one. `product_theming = false` stops each product tinting its own
pages.

Both colors must be `#rrggbb`, and setting `accent_dark` without `accent` is
an error. [Theming](~trail/building/theming) covers what these actually reach.

## What can live in the site root

The site root accepts `trail.toml`, root collections, the build output
directory, dot-entries (`.git` and friends), and **whatever `favicon`,
`custom_css`, `head_html`, `passthrough` and
[`[[merge]]`](~trail/structuring-a-site/merged-trees) name**. A setting naming
`assets/site.css` makes the whole `assets/` directory legal at the root — it
is the first path component that is tolerated. Anything else is a build error.

That last clause is the reason those three settings take paths rather than
inline content: naming a file is what makes it part of the site.
