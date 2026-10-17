---
title: Books
type: concept
description: A book is a formal, ordered document — a specification or manual — with a cover page, dotted section numbers derived from directory names, chapters nesting to any depth, and lettered appendices.
related:
  - trail/structuring-a-site/topics-and-folders
  - trail/writing/inline-references
  - trail/reference/directory-names
  - trail/writing/articles-and-frontmatter
---

A **book** is a document read in order and cited by section: a specification, a formal reference, a manual. Where a [topic](~trail/structuring-a-site/topics-and-folders) is a set of related pages, a book is one document that happens to be split across pages.

Books get three things topics do not: a cover page, stable section numbers derived from the directory structure, and appendices.

## The directory

A book is a directory named `<order>--<slug>.book`, sitting in a product, an anthology or a shelf:

```text
1--pgss.book/                    →  /demo/pgss   (the cover)
├── trail.toml
├── 1--introduction.md           §1   →  /demo/pgss/introduction
├── 2--logon/                    §2   a chapter
│   ├── trail.toml
│   ├── 1--overview.md           §2.1 →  /demo/pgss/logon/overview
│   └── 2--flow.md               §2.2
└── a1--references.md            Appendix A
```

Its `trail.toml` requires a title and a description:

```toml
# 1--pgss.book/trail.toml
title = "Peios Generic System Standards"
short = "PGSS"
description = "Cross-platform system standards from the Peios Project"
```

| Key | |
|---|---|
| `title` | Required. The full name, shown on the cover and in cards. |
| `description` | Required. The standfirst on the cover, the card text, and the `llms.txt` entry. |
| `short` | Optional. An abbreviation — usually an acronym — used where the full title will not fit: the sidebar heading, the pill on the book's card, and the line above the title on the cover. |
| `inline_ref` | Optional. Phrases that auto-link to this book, and which may carry a `§` section suffix in prose. See [Inline references](~trail/writing/inline-references). |

## Section numbers come from the directory names

This is the part worth reading twice. **A book's section numbers are its order prefixes**, joined with dots along the path from the book root:

```text
2--logon/            →  §2
  1--overview.md     →  §2.1
  3--errors/         →  §2.3
    1--codes.md      →  §2.3.1
```

Two consequences follow:

- **Number books from 1, not 100.** The hundreds convention used for [topic articles](~trail/structuring-a-site/topics-and-folders) would produce §100 and §200. In a book the prefix is the published section number, so `1--`, `2--`, `3--` is what you want.
- **Gaps show through, on purpose.** If `2--` is deleted the book goes §1, §3 — because in a specification, section numbers are addresses. Renumbering to close a gap silently invalidates every citation anyone has ever written; leaving the gap is the honest outcome. Renumber deliberately, or not at all.

Inside an article, `h2` and `h3` headings continue the numbering as real text: in §2.1, the first `h2` is §2.1.1 and its first `h3` is §2.1.1.1. Headings deeper than `h3` are not numbered. The numbers are rendered into the heading itself rather than drawn with CSS counters, so they can be searched, selected and copied.

## Chapters

A chapter is a plain `<order>--<slug>` directory — no type suffix, exactly like a [topic subfolder](~trail/structuring-a-site/topics-and-folders) — except that chapters may nest as deeply as the document needs.

A chapter:

- **contributes a URL segment and a section number**;
- **has no page of its own** — links to it open its first article;
- **must contain at least one article, somewhere beneath it.** An empty chapter is a build error, because it would appear in the contents as a number leading nowhere;
- takes an optional `trail.toml` with `title` and `inline_ref`, with the title otherwise derived from the slug.

## Appendices

An entry whose order is written `a<N>--` is an **appendix**: lettered instead of numbered, and sorted after every numbered sibling at its level.

```text
1--pgss.book/
├── 1--introduction.md      §1
├── 2--logon/               §2
├── a1--references.md       Appendix A
└── a2--glossary/           Appendix B
    └── 1--terms.md         §B.1
```

- Letters run `A`, `B`, … `Z`, `AA`, `AB`, and so on.
- The numbering is by `a<N>` order, not position: `a1--` is A, `a2--` is B. Appendix numbering starts at `a1`; `a0--` is an error.
- Appendices work at **any** level, not just the book root. An appendix chapter inside a numbered chapter gives §2.A, and its articles §2.A.1.
- Where the entry itself is displayed — the sidebar, the contents — it is labelled "Appendix A" in full. In section references it is just the letter.

## The cover page

A book's URL is its cover: the product and anthology breadcrumbs, the `short` name if it has one, the title, the description, and the book's total reading time and last-updated date. The contents are the sidebar tree beside it — the whole book, chapters as collapsible groups, section numbers on every entry.

The tree remembers which chapters you opened, per book, as you move between pages.

## Frontmatter is different inside a book

Book articles use a reduced frontmatter set. `title:` is still required; `description:`, `related:`, `inline_ref:`, `updated:` and `reading_minutes:` all still work.

**`type:` is an error inside a book.** The concept/how-to/reference taxonomy describes how to approach a page, and inside a formal document that question is already answered — you read it in order. Trail rejects the key rather than letting it sit there meaning nothing.

## Citing sections

Books are the reason [inline references](~trail/writing/inline-references) exist. A book that claims a phrase can be cited by section from anywhere in the site:

```markdown
Every token MUST carry a lifetime, per PGSS §2.1.
```

"PGSS" links to the book; "PGSS §2.1" links to the exact section, resolving through the book's index of section numbers — including numbers that belong to a heading inside an article, not just to the article itself. Within a book's own pages, a bare `§2.1` resolves the same way against the surrounding book.

Because a section number is an address, this is where the "gaps show through" rule earns its keep.

## When a book is the wrong answer

A book is a commitment: numbered sections that readers may cite, an order that means something, and a cover page. Most documentation is not that. If your sections would never be cited by number, and the pages can be read in any order, you want a topic.
