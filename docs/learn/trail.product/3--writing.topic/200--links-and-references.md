---
title: Links and references
type: how-to
description: The ~ reference grammar — link to a page by its identity rather than its URL, with the shortest unambiguous suffix. How resolution works, what can be a target, and what the build does when a link breaks.
related:
  - trail/writing/articles-and-frontmatter
  - trail/structuring-a-site/link-aliases
  - trail/writing/inline-references
  - trail/building/build-and-serve
---

Trail links pages by **identity**, not by URL. Instead of writing a path that breaks the moment a page moves, you write a `~` reference naming the product and enough of the page's slugs to be unambiguous:

```markdown
See [Commands and targets](~pekit/running/commands-and-targets) first.
```

Reorganise the tree — move that topic into an anthology, wrap it in a shelf, renumber it — and the link still resolves. Delete the target, and the build fails and tells you every page that pointed at it.

## The grammar

```text
~<product-slug>[/<segment>]…[#<fragment>]
```

- The **first segment is always a product slug**. There is no relative form and no site-wide search; every reference says which product it means.
- The **remaining segments match the end of a page's path.** Trail compares them against each page's slugs — the anthology slugs, the topic slug, any subfolder or chapter slugs, and the page's own — and matches if your segments are the tail of that list.
- Exactly one page must match. Zero or several is an error.
- A **fragment** may follow, and is appended to the resolved URL: `~pekit/reference/cli#global-flags`. It is checked too: the target article must define that anchor — a heading id, a `[*name]` citation anchor, or an id written in raw HTML — or the link is broken. Fragments on product and anthology pages, which have no body, are not checked.

The rule of thumb is: **write the shortest thing that is unambiguous**, and add segments from the left only when you have to.

### A worked example

Given this page:

```text
/peios/security-fundamentals/identity/tokens/issuing
        └ anthology ───────┘ └ topic ┘ └folder┘ └page┘
```

all of these resolve to it:

```text
~peios/issuing
~peios/tokens/issuing
~peios/identity/tokens/issuing
~peios/security-fundamentals/identity/tokens/issuing
```

`~peios/issuing` is the one to write — until a second page in `peios` is also called `issuing`, at which point that reference becomes ambiguous, the build fails, and you add a segment.

Note that the deeper segments are for *disambiguation only*. A chapter or subfolder slug can appear in a reference even though it is not a page you could link to on its own.

## What can be a target

| | Targetable | Reference |
|---|---|---|
| A product | yes | `~pekit` |
| An anthology | yes | `~peios/security-fundamentals` |
| A book cover | yes | `~peios/pgss` |
| An article | yes | `~pekit/reference/cli` |
| A topic | **no** | link to an article in it |
| A topic subfolder | **no** | *(except as a `.link` target)* |
| A book chapter | **no** | link to an article in it |
| A shelf | **no** | it has no page at all |

Topics, subfolders and chapters have no page of their own, so there is nothing for a reference to land on. Link to the article you actually mean.

## Ordinary links still work

A `~` at the start of a destination is what makes it a reference. Everything else is left alone:

```markdown
[the CommonMark spec](https://spec.commonmark.org)     external, untouched
[the sitemap](/sitemap.xml)                            root-relative, untouched
[this section](#the-grammar)                           same-page fragment
```

Root-relative links are the escape hatch for the handful of URLs that are not pages — `/print` views, `/llms.txt`, `/site.json`. They are not checked, so they are also the way to ship a broken link. Prefer `~` wherever the target is a page.

A same-page fragment *is* checked, against the page's own heading ids, citation anchors and raw-HTML ids. Heading ids are derived from the heading text — lowercased, every run of non-alphanumerics collapsed to one hyphen, so `## vm:run(cmd, opts)` is `#vm-run-cmd-opts` — and a repeated heading gets `-2`, `-3`, … appended. Inside a `/print` bundle the link is namespaced along with the ids it points at, so it keeps working there.

One form is always an error: a **relative path**, such as `../410--network-policy.md`. Pages are addressed by their slugs, not their source files, so no relative path can ever reach one; the build rejects it and tells you to write a `~` reference instead.

`~` is only meaningful in a *link* destination. In an [image](~trail/writing/images) destination it is an error, with a message saying so: images are files, addressed relative to the article.

## Where else references are used

The same grammar appears in three other places:

```yaml
# an article's frontmatter — no leading ~
related:
  - pekit/running/invocation
```

```toml
# a .link alias file — with the ~
target = "~pekit/running/signing-and-provenance"

# the root trail.toml's nav — with the ~, or an external URL
[[nav]]
label = "Specs"
url = "~peios/pgss"
```

The `related:` list is the odd one out in dropping the `~`; everything else keeps it.

## When a link breaks

Every link in the site is checked on every build, and all the failures are reported together — one build tells you the whole story rather than stopping at the first bad reference.

```text
Error: 3 broken links:
in article '/pekit/recipes/anatomy': link '~pekit/running/invokation' matches no page in product 'pekit'
in article '/peios/identity/sids': link '~pekti/overview' names unknown product 'pekti'
in article '/pekit/reference/cli': link '~pekit/targets' is ambiguous; candidates:
  ~pekit/running/commands-and-targets/targets
  ~pekit/reference/recipe-format/targets
```

There are five failure kinds, and they are not treated alike:

| | Means | With `--allow-dangling-links` |
|---|---|---|
| **Unknown product** | the first segment is not a product slug | warning |
| **No match** | nothing in that product ends with those segments | warning |
| **Missing anchor** | the page exists but defines no such `#fragment` — on a `~` reference or a same-page link | warning |
| **Relative path** | the destination is a file path, which no page can serve | warning |
| **Ambiguous** | two or more pages match | **still fatal** |

Ambiguity is never downgradable. A missing target is a link that does nothing; an ambiguous one is a link that goes somewhere *specific and possibly wrong*, and only the author knows which page was meant.

`--allow-dangling-links` is for the middle of a refactor, when you have moved a hundred pages and want to see the site before fixing every reference. It is not a setting to leave on.

## References in print views and mirrors

The [single-page and markdown views](~trail/building/the-output-surface) rewrite references so they still work away from the site:

- In a **`/print` bundle**, a reference to a page inside the same bundle becomes an in-document anchor, so the whole document navigates itself. A reference to a page outside it becomes an absolute URL when the site config has a `url`, and stays root-relative otherwise.
- In a **`.md` mirror**, a reference becomes the target's own `.md` URL, so a reader following links through the markdown surface stays in it.
