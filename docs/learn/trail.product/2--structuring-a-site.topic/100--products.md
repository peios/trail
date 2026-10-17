---
title: Products
type: concept
description: A product is the top-level division of a trail site — a documented thing with its own landing page, colour, monogram and URL prefix. What goes in one, what its trail.toml needs, and how products are ordered.
related:
  - trail/getting-started/anatomy-of-a-site
  - trail/structuring-a-site/anthologies-and-shelves
  - trail/structuring-a-site/topics-and-folders
  - trail/building/configuring-the-site
---

A **product** is the outermost division of a trail site: one documented thing, with its own landing page, its own colour, and its own URL prefix. Products are the only children the site root will accept, so every page in the site belongs to exactly one.

On this site, `pekit`, `provium` and `trail` are products. So is `peios` itself — products are not necessarily software packages, they are simply the largest chunk of documentation a reader would think of as one subject.

## The directory

A product is a directory named `<slug>.product` at the site root. It has no order prefix.

```text
learn/
├── trail.toml
├── pekit.product/          →  /pekit
└── trail.product/          →  /trail
```

The slug becomes the URL prefix (`/pekit`) and the first segment of every [`~link`](~trail/writing/links-and-references) into that product. It must be lowercase kebab-case, and it may not be `assets` or `pagefind` — those collide with the output directories trail writes.

## The config

Every product needs a `trail.toml`, and all four keys are required:

```toml
# pekit.product/trail.toml
title = "pekit"
monogram = "pk"
color = "#22c55e"

description = "The recipe-driven build and packaging tool — declarative recipes in; built, tested, packaged software out."
```

| Key | |
|---|---|
| `title` | The product's display name, used everywhere the product is named. Unlike other containers, this is not derived from the slug — a product always states it. |
| `monogram` | A very short string — usually two characters — shown on the product's tile, on the front-page card and at the top of the product page. Initials or a shortened name. |
| `color` | A `#rrggbb` hex colour. Any other form is a build error. |
| `description` | One sentence, shown on the front-page card, under the product's name on its own page, and in `llms.txt`. It is also the product page's meta description. |

One optional key, `inline_ref`, claims phrases that auto-link to the product page wherever they appear in prose; see [Inline references](~trail/writing/inline-references).

## What the colour does

`color` is the product's accent, and by default it displaces the site accent on every page belonging to that product: card borders and hover states, the tile, sidebar highlights, heading numbers, RFC-2119 keywords, focus rings.

Setting `product_theming = false` in the root `trail.toml` turns this off site-wide, and everything tints from the site accent instead. Products still keep their monogram and card, they just stop colouring their pages. See [Theming](~trail/building/theming).

## What a product contains

A product directory holds its `trail.toml` and nothing but grouping directories:

- [**Anthologies**](~trail/structuring-a-site/anthologies-and-shelves) (`.antho`) — a titled landing page with its own children, for products big enough to need a second level.
- [**Topics**](~trail/structuring-a-site/topics-and-folders) (`.topic`) — a set of articles read in any order. The everyday container.
- [**Books**](~trail/structuring-a-site/books) (`.book`) — a formal, ordered document with a cover page, numbered sections and appendices.
- [**Shelves**](~trail/structuring-a-site/anthologies-and-shelves) (`.shelf`) — a titled heading grouping some of the above on the product page. No URL segment of its own.

Articles, images and `.link` files may **not** sit directly in a product directory; they live inside topics and books. A product with nothing in it is legal and simply renders an empty landing page.

Small products go straight to topics:

```text
pekit.product/
├── trail.toml
├── 1--getting-started.topic/
├── 2--recipes.topic/
├── 3--running.topic/
└── 4--reference.topic/
```

Large ones add a layer:

```text
peios.product/
├── trail.toml
├── 1--security-fundamentals.antho/
├── 2--using-peios.antho/
├── 3--advanced-peios.antho/
├── 4--developing-for-peios.antho/
└── 5--peiso.antho/
```

Both shapes are ordinary. Reach for an anthology when a product's topic list has grown long enough that readers have to hunt through it.

## The product page

Every product gets a landing page at `/<slug>`, built from the config and its children:

- A hero with the monogram tile, the title and the description.
- A "Documentation" section listing every child as a card. Anthologies and books get a card with their description; topics get a card listing their articles — all of them when there are four or fewer, otherwise the first three and a "See all N articles" link — with a count and the topic's total reading time.
- Each shelf becomes a subheading above its own row of cards. Runs of items outside any shelf render as an untitled row, in place.

## Ordering

Products have no order prefix. They sort by the root `trail.toml`'s `featured` list — in that list's order — and then everything not listed, by title.

```toml
# learn/trail.toml
featured = ["peios", "pekit", "provium", "universal-directory", "trail"]
```

That list does more than sort. **The front page shows only featured products.** An unfeatured product is still built, still searchable, still linkable, and still in the header's products menu — it just does not get a card on the front page. Naming a product that does not exist in `featured` is a build error.

## Roll-ups

A product totals up what is beneath it. Its reading time is the sum of every article's, and its `updated` date is the most recent `updated:` in the whole product. Anthologies, topics, books and chapters do the same at their own level. See [Articles and frontmatter](~trail/writing/articles-and-frontmatter).

## When to add a product

A product is a heavy division: its own colour, its own landing page, its own URL prefix, and a separate card on the front page. Add one when readers would go looking for the subject by name and would not expect to find it filed under something else.

Within a single product, the lighter divisions — anthologies for a second level of landing pages, shelves for a heading, topics for a set of articles — are almost always the better answer.
