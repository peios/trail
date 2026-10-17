---
title: Inline references
type: concept
description: A page can claim the phrases that mean it, and every occurrence in the site's prose becomes a link to it. Plus § section citations into books, and automatic RFC-2119 keyword highlighting.
related:
  - trail/writing/links-and-references
  - trail/structuring-a-site/books
  - trail/writing/articles-and-frontmatter
  - trail/reference/frontmatter
---

Some terms should always be links. If there is one page explaining what a PCDS GUID is, then every "PCDS GUID" written anywhere on the site ought to point at it — and nobody wants to write that link by hand a hundred times, or notice the ninety-nine places it was forgotten.

An **inline reference** is a phrase a page claims. Trail then links every occurrence of that phrase in the site's prose to the claiming page, automatically.

## Declaring a phrase

Declaration is **self-service**: a thing declares the phrases that mean it, in its own file. There is no central glossary to maintain.

An article declares in its frontmatter:

```yaml
---
title: Globally unique identifiers
type: reference
inline_ref:
  - PCDS GUID
  - PCDS GUIDs
---
```

Everything else declares in its `trail.toml`:

```toml
# 2--pgss.book/trail.toml
title = "Peios Generic System Standards"
short = "PGSS"
description = "Cross-platform system standards from the Peios Project"
inline_ref = ["PGSS", "Peios Generic System Standards"]
```

Phrases are matched **literally**, so plurals and alternative spellings are separate claims. List them all.

## Where a phrase lands

| Declared on | Links to |
|---|---|
| An article | that article |
| A product | the product's landing page |
| An anthology | the anthology's landing page |
| A book | the book's cover — and `§` may reach a section |
| A topic | the topic's first article |
| A topic subfolder | the folder's first article |
| A book chapter | the chapter's first article |

The pattern is: the declarer's own page, or — for the containers that have no page — the first article inside it. A container that declares a phrase but holds no articles to land on is a build error.

**A shelf may not declare a phrase at all**, since it has neither a page nor articles of its own. Declare it on the shelf's items.

## Where phrases are matched

In ordinary prose, and only there. Matching skips:

- **Headings**, whose ids are derived from their text;
- **Link text** — a phrase inside an existing link is left alone, so a deliberate link is never rewritten;
- **Image alt text**;
- **Code blocks and inline code** — writing `` `PCDS GUID` `` when you mean the literal string produces no link;
- **The claiming page itself**, which never links to itself.

Two more rules make matching predictable:

- **Whole words only.** A phrase surrounded by letters, digits or underscores does not match. "PCDS GUIDs" does not match inside "PCDS GUIDsomething".
- **Longest wins.** When two claimed phrases overlap at the same position, the longer one is used — so "PCDS GUID" beats "PCDS", and a phrase does not get chopped up by a shorter one inside it.

## Phrases are exclusive

A phrase belongs to exactly one declarer, site-wide. Two things claiming the same phrase is a build error:

```text
inline_ref phrase 'PCDS' is claimed by both book '/peios/pcds' and article '/peios/formats/pcds-overview'
```

That is the whole point: if two pages both want to own a term, the ambiguity is real and a reader would have been sent to the wrong one. Decide which page is the destination.

Empty phrases, phrases with leading or trailing whitespace, and phrases containing `§` are also errors. [Link aliases](~trail/structuring-a-site/link-aliases) never claim phrases — the original keeps them.

## Choosing phrases well

Inline references are powerful in the way find-and-replace is powerful. A phrase that is too common turns the site into a sea of blue.

- **Claim terms of art, not ordinary words.** "PCDS GUID" is a term. "identity" is a word.
- **Claim what a reader would want defined.** The test is whether someone meeting the phrase mid-sentence would benefit from a link.
- **Prefer the specific form.** Claiming "GUID" catches every passing mention; claiming "PCDS GUID" catches the ones that mean your page.
- **Remember plurals and expansions** — they are separate claims, and a term claimed in only one of its forms links inconsistently, which reads worse than not linking at all.

## Section citations with §

A phrase claimed by a **book** can be extended with a section number in prose:

```markdown
Every token MUST carry a lifetime, per PGSS §2.1.
```

"PGSS §2.1" becomes one link to that exact section. The suffix is a single space (or non-breaking space), the `§` sign, and a section label — digits, dots and appendix letters: `§2`, `§2.1.3`, `§A`, `§B.2`. A sentence-ending full stop is not part of the label.

Every book carries an index of its section numbers, covering:

- **articles**, by their section number (`§2.1` → that article's page);
- **chapters**, by theirs (`§2` → the chapter's first article);
- **headings inside articles** — the `h2` and `h3` numbers trail generates (`§2.1.3` → that article, at that heading).

So a citation can be as precise as a subsection of a page.

A `§` label that does not resolve is a build error — with the phrase still linked to the book, so the page is not broken while you fix it:

```text
'PGSS §9.9' does not match any section of '/peios/pgss'
```

Like a broken `~link`, it is downgraded to a warning by `--allow-dangling-links`.

### Bare § inside a book

Within a book's own pages, a bare `§` reference resolves against the surrounding book, with no phrase needed:

```markdown
The requirements of §2.2.1 apply, except as noted in §A.
```

Unlike the phrase-qualified form, **a bare `§` that does not resolve stays plain text and says nothing.** That is not an oversight: specifications routinely cite sections of *other* documents, and a build that failed on every "§9.9 of RFC 3986" would be unusable. The qualified form names its book and so can insist; the bare form cannot, so it does not.

## RFC-2119 keywords

Requirement keywords are highlighted everywhere on the site, with no configuration and nothing to declare:

> MUST · MUST NOT · REQUIRED · SHALL · SHALL NOT · SHOULD · SHOULD NOT · RECOMMENDED · NOT RECOMMENDED · MAY · OPTIONAL

They must be written in capitals, as [RFC 2119](https://www.rfc-editor.org/rfc/rfc2119) intends — the capitals are what distinguishes a requirement from the ordinary English word. Matching is whole-word and longest-first, so "MUST NOT" highlights as one keyword rather than as "MUST" followed by a stray "NOT", and it skips the same places phrase matching skips: headings, links, and code.

The styling is a weight and the product's accent colour, not a badge. It is meant to make a requirement scannable in a page of prose, and it works in ordinary articles as well as in books.
