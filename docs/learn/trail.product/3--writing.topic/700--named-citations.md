---
title: Named citations
type: concept
description: A `[*name]` anchor names one statement inside an article so a test, a code comment or a coverage report can point at it — and so a change to the statement can find everything that depended on it.
related:
  - trail/writing/inline-references
  - trail/writing/links-and-references
  - trail/building/the-output-surface
---

A `§` reference reaches an article. That is usually the right size for prose — but not for a test. A test verifies one statement, and an article holds many; "this test covers §4.6" cannot tell you which of §4.6's nine statements is still unverified.

A **named citation** is an anchor on a single statement:

```markdown
Copy-up preserves the POSIX ownership of the object as provided by
the lower stratum. [*copy-up.preserves-ownership]
```

Anything outside the site can now point at exactly that sentence:

```rust
// Verifies PKM *copy-up.preserves-ownership
```

The build collects every anchor into `citations.json`, so a test suite can ask which citations nothing yet verifies.

## Anchors on headings

An anchor on a heading names the **whole section**:

```markdown
### `ro` [*strata.ro-is-independent-of-filesystem]
```

This is usually what you want. A well-structured document already gives
a behaviour its own section, so the section is the citable unit, and an
anchor on a heading rides the structure rather than one sentence of
prose that may later be rewritten.

The marker renders inside the heading and contributes nothing to the
heading's id or its table-of-contents entry, so it is safe to add to an
existing heading — nothing that linked to that section breaks.

In `citations.json` a heading anchor is reported with `"kind":
"section"` and its context is the heading text; everything else is
`"kind": "statement"`. A coverage report wants the difference: an
uncovered section is a different-sized gap from an uncovered sentence.

Reach for a statement anchor when one section states several
independent rules — a table of error conditions, or a numbered list —
where one name for the section would be too coarse to say which rule is
untested.

## An anchor is a locator

It marks a **place**, not a span. `[*copy-up.preserves-ownership]` says "the statement here is called this"; it does not say where that statement begins or ends, and Trail never tries to work that out.

That is deliberate. A name is arbitrary, so nobody argues about it and nobody revises it. An extent *looks* like it has a correct answer — does the following clarifying sentence belong inside? the exception clause? — so it invites disagreement and revision, and every revision is churn that helps none of the things a citation is for. Overlapping and nested statements have no clean bracketing rules either.

So an anchor is a pointer, and a reader follows it and reads.

> [!NOTE]
> If a paragraph holds three statements you cannot tell apart, **write three paragraphs**. That is a prose improvement rather than a workaround, and it makes each anchor's context exact for free. The same reasoning applies one level up, where an article covering nine unrelated rules forces every `§` citation to be approximate.

## One citation, many tests

Coverage is *"does this citation have at least one passing test"*. It is never a one-to-one mapping.

A statement in a specification is often close to one testable behaviour. In a reference manual it usually is not: an anchor is as much a way of dividing a large descriptive document into consumable chunks as it is a single assertion. Getting closer to one-anchor-one-behaviour is better where it happens naturally, but it is not a requirement, and a report that assumes it will mislead.

## Writing anchors

Put the anchor at the end of the statement it names. Do it as a **separate pass**, after the prose is written — annotating while drafting interrupts the writing, and picking out testable statements is a different kind of attention from composing them.

A name is lowercase alphanumerics in dot- or hyphen-separated segments, starting and ending alphanumeric:

| | |
|---|---|
| `copy-up.preserves-ownership` | ✅ |
| `mount.strata.order` | ✅ |
| `Copy-Up.Ownership` | ❌ uppercase |
| `copy..up` | ❌ doubled separator |
| `.leading` | ❌ must start alphanumeric |

The grammar is enforced at build rather than merely recommended: a name is what every citation greps for, so a corpus with three naming styles is a corpus where a search misses things.

**A name must be unique within its book**, and the build fails if one is declared twice. Uniqueness is per book rather than site-wide — unlike an `inline_ref` phrase — because names are naturally book-scoped and a citation from outside is qualified by the book's short name anyway.

**Only a book may carry an anchor.** A citation is addressed through its book, so an anchor in a task-doc topic could never be cited; the build rejects it rather than leaving it silently unreachable.

### Retiring one

Delete the anchor, and grep the name to find the tests and comments that pointed at it. Do not reuse a retired name for a different statement: nothing can detect that, and every existing citation silently comes to mean something else.

## How it renders

A small superscript marker, positioned out of the text's flow so line height and wrapping are exactly as they would be without it. Clicking copies the full citation string — the thing you want in a test — rather than navigating; the link still targets the anchor, so "copy link address" and middle-click behave normally, and the page works without JavaScript.

Readers who would rather not see them at all can call `trailToggleCitations()`, which persists per browser.

## What the build produces

`citations.json`, at the site root, listing every anchor in the corpus:

```json
{
  "citations": [
    {
      "name": "copy-up.preserves-ownership",
      "citation": "PKM *copy-up.preserves-ownership",
      "book": "/peios/advanced-peios/peios-kernel",
      "article": "/peios/advanced-peios/peios-kernel/stratafs/overview",
      "url": "/peios/advanced-peios/peios-kernel/stratafs/overview#cite-copy-up.preserves-ownership",
      "section": "4.1",
      "context": "Copy-up preserves the POSIX ownership of the object as provided by the lower stratum."
    }
  ]
}
```

It is its own file rather than part of `site.json`, which is a tree of the whole site: the consumers here want a list they can read without walking one.

`context` is **the source block the anchor sits in**, not the statement it marks — an anchor delimits nothing. Two anchors in one paragraph get the same context. Treat it as a label for a coverage report, and follow `url` when you need the real thing.

Anchors are also left as written in the markdown mirrors, so an agent reading `print.md` sees them.

## Anchors in code samples stay literal

`[*name]` inside a fenced block or inline code is never an anchor, which is what lets this page show the syntax. Everywhere else, an anchor is rewritten before the markdown is parsed — otherwise the `*` could pair with an emphasis marker elsewhere in the paragraph and swallow the anchor into an `<em>`.
