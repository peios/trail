---
title: trail.toml reference
type: reference
description: Every key of every trail.toml — the site root, products, anthologies, books, topics, subfolders, chapters and shelves — with types, defaults, validation rules and what each one reaches.
related:
  - trail/building/configuring-the-site
  - trail/reference/directory-names
  - trail/reference/frontmatter
  - trail/building/theming
---

`trail.toml` appears at every level of the tree, with a different schema at each. **Unknown keys are build errors everywhere**, so a typo fails loudly instead of doing nothing.

## The site root

Required at the site root. Four keys are mandatory.

| Key | Type | Default | |
|---|---|---|---|
| `sitename` | string | **required** | The site's identity: header wordmark (last word accented), `<title>` suffix, search placeholder, `og:site_name`. |
| `title` | string | **required** | The front page's headline; its `og:title` and the base for its meta description. |
| `description` | string | **required** | The front page's standfirst and meta description; the first line of `llms.txt`. |
| `footer` | string | **required** | The line at the foot of every page. |
| `url` | string | none | The site's public base URL, e.g. `https://learn.peios.org`. Trailing slashes are trimmed. |
| `featured` | array of strings | `[]` | Product slugs, in order. Sorts products, and selects which appear on the front page. |
| `accent` | `#rrggbb` | theme default | The site accent colour. |
| `accent_dark` | `#rrggbb` | derived | The dark-mode accent. Derived by mixing `accent` 75% toward white when absent. |
| `custom_css` | path | none | A stylesheet shipped at `/assets/custom.css`, loaded after the built-in one. |
| `favicon` | path | none | Shipped at the output root under its own name, linked from every page. |
| `head_html` | path | none | A file injected verbatim at the end of every page's `<head>`. |
| `edit_url` | string | none | "Edit this page" URL template. Must contain `{path}`. |
| `nav` | array of tables | `[]` | Header links. See below. |
| `passthrough` | array of paths | `[]` | Root entries copied into the output verbatim. See below. |
| `product_theming` | bool | `true` | Whether each product tints its own pages. |
| `built_by_trail` | bool | `true` | Whether "Built with Trail." appears under the footer. |
| `plugins` | tables | none | Commands that expand [plugin blocks](~trail/writing/plugin-blocks). See below. |
| `merge` | array of tables | `[]` | Other trees whose products join the site. See below. |
| `lint` | table | none | The writing rules `trail lint` checks. See [Checking style](~trail/writing/checking-style). Builds ignore it, but a malformed table fails them. |

```toml
sitename = "Peios Learn"
title = "Documentation for everything Peios"
description = "Concepts, guides, and reference for the Peios operating system…"
url = "https://learn.peios.org"
accent = "#3b82f6"
featured = ["peios", "pekit", "provium", "universal-directory", "trail"]
footer = "Peios Learn — documentation for the Peios project."

[[nav]]
label = "Specs"
url = "~peios/pgss"
```

### Validation

- `accent` and `accent_dark` must be `#rrggbb`. `accent_dark` without `accent` is an error.
- `custom_css`, `favicon` and `head_html` are paths relative to the site root, and **the file must exist**. Naming one is also what makes it legal to keep in the site root; the first path component is what is tolerated there.
- `edit_url` must contain the literal `{path}`, which is replaced with the article's source path relative to the site root (or, for a page in a [merged tree](~trail/structuring-a-site/merged-trees), relative to that tree's root, using that tree's `edit_url`).
- Every slug in `featured` must name an existing product.

### `passthrough`

Root entries trail neither builds nor understands, copied into the output as they are:

```toml
passthrough = ["CNAME", ".well-known", "ads.txt"]
```

Each entry is a plain relative path naming a file or a directory that exists in the site root; directories are copied whole, nesting included. Naming an entry is also what makes it **legal** to keep in the site root, exactly as `favicon` and friends do — so this is how a repository that is also a trail site keeps its `CNAME` beside its `trail.toml`.

Errors: an entry that is empty, absolute, or contains `..`; one naming something that does not exist; one naming the build output directory.

Passthrough entries are copied **last**, after everything trail generates, so an entry deliberately named after a generated file replaces it. Naming `robots.txt` is a supported way to ship your own; naming `assets` would bury the stylesheet, and trail will not stop you.

### `[[nav]]`

| Key | Type | |
|---|---|---|
| `label` | string | **required.** The link text. |
| `url` | string | **required.** An external URL, a root-relative path, or a [`~` page reference](~trail/writing/links-and-references). |

References here are resolved **strictly at load time** — a broken nav link fails the build even under `--allow-dangling-links`, because site chrome appears on every page.

> [!IMPORTANT]
> `[[nav]]` blocks must come **last in the file**. TOML gives every key after a table header to that table, so a `footer =` written below them is read as a nav item's field and fails with `unknown field 'footer', expected 'label' or 'url'`. Put every plain key above the first `[[nav]]`.

A shelf is not a link target, so a nav entry cannot point at one — aim at the anthology above it, or a page inside it.

### `[plugins.<name>]`

One table per [plugin](~trail/writing/plugin-blocks), keyed by the name its blocks use (`{name}`).

| Key | Type | |
|---|---|---|
| `command` | array of strings | **required.** The program and its arguments, run without a shell, from the site root. A program containing `/` is a path relative to the site root; a bare name is looked up on `PATH`. |
| `anchors` | bool | Default `false`. Send the plugin [every page's anchors](~trail/writing/plugin-blocks#linking-to-the-rest-of-the-site), so it can link to targets that other pages define. |
| `after` | array of strings | Default none. Plugins that run before this one, whose output's anchors join this one's anchors. Needs `anchors = true`. |

```toml
[plugins.kernel-doc]
command = ["python3", "../tools/docs/trail-kernel-doc/trail-kernel-doc", "--srctree", ".."]
anchors = true

[plugins.kernel-include]
command = ["python3", "../tools/docs/trail-kernel-doc/trail-kernel-include", "--srctree", ".."]
anchors = true
after = ["kernel-doc"]
```

Errors:
- a name other than lowercase letters, digits, `-` and `_`;
- an empty `command`;
- a program path that does not exist;
- `after` without `anchors = true`, naming a plugin that is not configured, or forming a cycle.

A program path inside the site root is tolerated there, the same way `favicon` and friends are.

Like `[[nav]]`, plugin tables must come **after every plain key**, since TOML gives any key below a table header to that table. In what order `[[nav]]`, `[plugins.*]` and `[[merge]]` come relative to each other does not matter.

In a [merged tree](~trail/structuring-a-site/merged-trees)'s `trail.toml`, "the site root" in all of the above means that tree's root: its plugins resolve and run there.

### `[[merge]]`

One table per [merged tree](~trail/structuring-a-site/merged-trees), in the order its plugins and passthrough entries yield to one another.

| Key | Type | |
|---|---|---|
| `path` | string | **required.** The tree's directory, relative to the site root. |

```toml
[[merge]]
path = "../kernel/docs"
```

Every `*.product` in the tree joins the site. A merged tree's `trail.toml` is optional; when present it must be valid, and only its `edit_url`, `passthrough` and `plugins` apply. Everything else, including its own `[[merge]]` list, is ignored.

Errors:
- an empty `path`, or one that is not a directory;
- a `path` that is the site root itself;
- a product slug that two trees share;
- anything in the tree's root that would be an error in a site root, except a `dist/` directory.

A `path` inside the site root is tolerated there, the same way `favicon` and friends are.

### What `url` unlocks

Without it: no `sitemap.xml`, no `Sitemap:` line in `robots.txt`, no `og:url`, relative canonical URLs, and relative outward links in `/print` bundles. Set it before deploying.

## A product

Required in every `<slug>.product` directory.

| Key | Type | |
|---|---|---|
| `title` | string | **required.** The product's display name. |
| `monogram` | string | **required.** The short string on the product's tile. |
| `color` | `#rrggbb` | **required.** The product accent. |
| `description` | string | **required.** One sentence: the card text, the page standfirst, the `llms.txt` entry. |
| `inline_ref` | array of strings | Phrases linking to the product's landing page. |

```toml
title = "pekit"
monogram = "pk"
color = "#22c55e"
description = "The recipe-driven build and packaging tool."
inline_ref = ["pekit"]
```

## An anthology

Required in every `.antho` directory.

| Key | Type | |
|---|---|---|
| `title` | string | **required.** |
| `description` | string | **required.** The card text, the page standfirst, the `llms.txt` entry. |
| `inline_ref` | array of strings | Phrases linking to the anthology's landing page. |

## A book

Required in every `.book` directory.

| Key | Type | |
|---|---|---|
| `title` | string | **required.** |
| `description` | string | **required.** |
| `short` | string | An abbreviation used where the full title will not fit: the sidebar heading, the card pill, the line above the cover title. |
| `inline_ref` | array of strings | Phrases linking to the book's cover. These may carry a `§` section suffix in prose. |

```toml
title = "Peios Generic System Standards"
short = "PGSS"
description = "Cross-platform system standards from the Peios Project"
inline_ref = ["PGSS", "Peios Generic System Standards"]
```

## A topic, subfolder or chapter

Optional. Both keys are optional.

| Key | Type | |
|---|---|---|
| `title` | string | Overrides the slug-derived title. |
| `inline_ref` | array of strings | Phrases linking to the container's **first article**. Declaring them on a container with no articles is an error. |

```toml
title = "Reference"
```

## A shelf

Optional, and takes `title` only.

| Key | Type | |
|---|---|---|
| `title` | string | Overrides the slug-derived title. |

**`inline_ref` on a shelf is an error.** A shelf has no page and no articles of its own, so the phrase would have nowhere to land — declare it on the shelf's items instead.

## Derived titles

Wherever `title` is optional and absent, it comes from the slug: hyphens become spaces, and the words take title case. The first and last words and every major word are capitalised. Between them, these stay lowercase:

- articles: a, an, the;
- coordinating conjunctions: and, but, or, nor, for, so, yet;
- "as";
- prepositions, such as to, of, in, on, at, by, from, with, about, between, through and without.

So `2--writing-tests.topic` becomes "Writing Tests", `developing-for-peios` becomes "Developing for Peios", and `what-an-event-is` becomes "What an Event Is".

Words that are as often adverbs or particles as prepositions, such as "up", "off" and "over", are capitalised: a slug can't say which it means, and "Setting Up the Network" needs the capital.

Write the key when the derived title is wrong. A slug is lowercase, so it can't carry acronyms, proper nouns or a name's own casing ("GXWI", "netd").

## `inline_ref` everywhere

The key means the same thing at every level: a list of phrases that should become links to this thing wherever they appear in the site's prose. Phrases are exclusive site-wide — two declarers claiming one phrase is an error — and must not be empty, carry leading or trailing whitespace, or contain `§`. See [Inline references](~trail/writing/inline-references).
