---
title: Link aliases
type: how-to
description: A .link file places an existing page in a second location — same content, second URL, canonical pointing home. How to write one, what it may target, and why the alias stays out of the search index.
related:
  - trail/structuring-a-site/topics-and-folders
  - trail/structuring-a-site/books
  - trail/writing/links-and-references
  - trail/reference/directory-names
---

Some pages belong in two places. A "Signing and provenance" article that lives in pekit's *Running* topic is also the page a reader of the security anthology goes looking for; a specification's terminology section is also an appendix of the manual next to it.

A **`.link` file** places an existing page at a second location. The content is not copied — it is the same article, rendered again at a second URL, with its canonical URL still pointing at the original.

## Writing one

A link is a file named `<order>--<slug>.link`, holding TOML:

```toml
# 3--signing.link
target = "~pekit/running/signing-and-provenance"
title = "Signing and provenance"
```

| Key | |
|---|---|
| `target` | Required. A [`~` page reference](~trail/writing/links-and-references), in exactly the grammar body links use. It must start with `~`. |
| `title` | Optional. The title the aliased page carries *here*. Without it, the title is derived from the link's own slug. |

The `<order>--<slug>` prefix works exactly as it does for an article: the order places the alias among its siblings, and the slug becomes its URL segment. The alias does not inherit the target's slug, its order, or its title — it is a new entry in a new place that happens to render the target's body.

## Where a link may live

`.link` files sit wherever articles sit: in a topic, in a topic subfolder, in a book, or in a book chapter. They may not sit in a product, an anthology or a shelf.

What each may target:

| In a | May target |
|---|---|
| Topic | an article, **or a whole topic subfolder** |
| Topic subfolder | an article |
| Book or chapter | an article |

Aliasing a subfolder from a topic clones the entire folder into that topic's URL space — every article in it becomes an alias of its own original, under the new folder's slug. It is the one case where a single link file produces more than one page.

A link may never target another link. That would make the canonical chain ambiguous, so trail rejects it.

## What the alias page is

The aliased page renders exactly like the original: same body, same images, same headings. What differs:

- **Its URL and title** are the link's, not the target's.
- **Its canonical URL is the original's.** Search engines are told which of the two URLs is the real one, so the duplicate does not compete with it.
- **It is not in the search index.** Searching finds the original; the alias exists for people navigating the tree.
- **It claims no `inline_ref` phrases.** The original keeps them — a phrase must have exactly one destination.
- **"Edit this page" points at the original's source file**, because that is the file to edit.
- Inside a book, an alias gets a section number from its own order prefix like any other entry, may be an appendix (`a<N>--`), and carries no `type:` or `description:` — book entries never do.

## Example

Pekit's signing article lives in its *Running* topic. The security anthology wants it too:

```text
pekit.product/3--running.topic/
└── 600--signing-and-provenance.md        →  /pekit/running/signing-and-provenance

peios.product/1--security-fundamentals.antho/4--supply-chain.topic/
├── 100--overview.md
└── 200--pekit-signing.link               →  /peios/security-fundamentals/supply-chain/pekit-signing
```

```toml
# 200--pekit-signing.link
target = "~pekit/running/signing-and-provenance"
title = "Signing in pekit"
```

Both URLs work. Both show the same prose. Search returns the pekit one. Editing either takes you to the same source file.

## When not to use one

An alias is right when one piece of writing genuinely belongs in two navigational places. It is the wrong tool for:

- **A cross-reference.** If you just want to point at another page, write a [`~link`](~trail/writing/links-and-references) in the prose, or add it to the article's `related:` list.
- **A page that should differ between the two locations.** An alias is the same article; there is no way to vary it. Write two articles.
- **A rename.** Aliases are not redirects — the old URL still has to exist as a `.link` file forever. If a page has moved for good, move it and fix the links; trail will tell you which ones broke.
