---
title: The Reading Experience
type: concept
description: Everything the theme gives a reader — search, three-state theming, the sidebar and On this page, text size, copy buttons, permalinks, prev/next, the single-page link and the mobile drawer.
related:
  - trail/building/the-output-surface
  - trail/building/theming
  - trail/writing/code-blocks
  - trail/building/configuring-the-site
---

None of this needs configuring. It is what every trail site does, and it is
here so you know what your readers have — and so you can write pages that make
use of it.

## Search

<kbd>⌘K</kbd> — or <kbd>Ctrl K</kbd>, and the pills in the header say which,
based on the reader's platform — opens a search modal from any page. So does
selecting the search box in the header or the one on the front page.

Search is powered by a [Pagefind](https://pagefind.app) index built at build
time. **The engine and its index chunks load lazily, on the first open**, so a
reader who never searches pays nothing for it.

- Results show the page title, its breadcrumb trail, and an excerpt with the
  matches highlighted.
- Up to eight results are shown; the status line says how many more there
  were.
- <kbd>↑</kbd> and <kbd>↓</kbd> move; <kbd>Enter</kbd> opens; <kbd>Esc</kbd>
  or a select on the backdrop closes.
- **The query is carried through to the page**, which highlights every
  occurrence of each term in the article body and scrolls the first into view.
  Matches inside code blocks and breadcrumbs are skipped.

What is indexed is the article body. Chrome — breadcrumbs, the meta line,
Related Content, the pager, the edit link — is excluded, and so is the front
page and every [alias page](~trail/structuring-a-site/link-aliases).

## Commands

A query beginning with `/` is a command rather than a search. Typing `/` on
its own lists the ones this page has; typing more narrows the list, and
<kbd>Enter</kbd> runs the highlighted one.

| Command | Meaning |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `/theme` | Cycles **system → light → dark**, exactly as the header button does. |
| `/theme <name>` | Goes straight to `system`, `light`, or `dark`. |
| `/cachebust`, `/cb` | Opens the page you are on in [the cache-busting copy](~trail/building/the-cache-busting-copy) — a URL nothing has cached. From inside the copy it takes you back out to the real page. Offered only when the build published a copy. |

Nothing needs enabling. The commands a page offers are the ones that can do
something there, which is why `/cb` appears on a site built with `--cbpath`
and nowhere else.

## Light, dark, and system

The header's theme button cycles **system → light → dark**, and its icon shows
which state is active. The choice is remembered in the reader's browser and
applied before first paint, so there is no flash of the wrong theme.

"System" is the default and follows the operating system's preference live.
See [Theming](~trail/building/theming).

## Navigation

**The sidebar** on an article page shows the article's own topic — its title
above its contents in reading order, subfolders as collapsible groups
(expanded automatically around the current page), and the current article
marked. In a [book](~trail/structuring-a-site/books) it is the whole book as a
tree with section numbers, chapters collapsible, and the trail to the current
page opened. Chapters a reader opens by hand stay open as they move through
the book, for that tab.

**"On this page"** lists the article's `h2` and `h3` headings in a right-hand
column, and highlights the one in view as the reader scrolls. When the window
is too narrow for a third column it moves into the sidebar. An article with no
headings gets no list.

**Every heading has a permalink** — a `#` that appears on hover and links to
that heading's stable id.

**Previous and next** links at the foot of an article step through the whole
topic, or the whole book, in reading order.

**Breadcrumbs** run from the product down through anthologies, the topic and
any subfolder.

**A back-to-top button** appears once the page has been scrolled past about
600 pixels.

## Text size

Three buttons at the top of the right-hand column set the prose to small,
medium or large. The choice is remembered per reader.

It scales the **article text only** — the sidebar, header, and chrome stay
put.
Making the whole page bigger costs a reader half their screen; making the
prose bigger is what they actually wanted. On narrow screens the controls ride
into the drawer with the rest of the sidebar.

## Code and content

- **Every code block has a copy button**, top-right, appearing on hover and
  always visible on touch devices. It says "Copied" briefly, and falls back to
  telling the reader to press <kbd>⌘C</kbd> if the browser refuses clipboard
  access.
- **Syntax highlighting follows the theme**, because it is class-based rather
  than baked-in colors.
- **Tables scroll inside their own container**, so a wide table never makes
  the whole page scroll sideways.
- **Images carry their real dimensions**, so text does not jump around as they
  load.
- **[Tab groups](~trail/writing/admonitions-tabs-and-diagrams)** keep one
  panel open at a time, independently per group.
- **[Mermaid diagrams](~trail/writing/admonitions-tabs-and-diagrams)** are
  drawn in the reader's theme, and the library is only shipped to pages that
  contain one.

## Line breaks

Some things read wrongly split across two lines, so Trail keeps them on one:

- **A spaced em dash stays with the word before it.** In "the kernel — not the
  service — decides", a line never starts with `—`. The space before the dash
  becomes a non-breaking space, including where the source breaks a line
  before the dash.
- **A numeric range stays whole.** "0–65535" never splits after its en dash.
- **A key combination stays whole.** <kbd>Ctrl</kbd>+<kbd>C</kbd> never splits
  at its "+".

This happens only in the rendered page. Sources keep plain spaces, because a
non-breaking space looks like an ordinary one but searches and compares
differently. Code spans and code blocks are left exactly as written. A range
or a key combination is wrapped in `<span class="keep-together">`, which a
[custom stylesheet](~trail/building/theming) can restyle. An invisible joining
character would have ridden along into anything a reader copies.

## Page furniture

**A meta line** under the title gives the estimated reading time and, when the
article has an `updated:` date, when it last changed. Containers total theirs
up, so a book cover says how long the whole book takes.

**A "Single page" link** in the breadcrumbs opens the whole topic or book as
one page, anchored at the article the reader was on. It is the right link for
someone who wants to read the lot, print it, or search the whole document at
once. See [The output surface](~trail/building/the-output-surface).

**"Edit this page"** appears at the foot of every article when the site config
sets `edit_url`, pointing at the article's source file.

**A "Related Content" list** appears above the pager when the article's
frontmatter has a `related:` list.

## On small screens

The sidebar becomes a drawer, opened by the menu button in the header and
closed by the overlay, the button again, or following any link inside it. "On
this page" and the text-size controls ride in with it, so nothing is lost on a
phone.

## On paper

Every page prints. The header, footer, search modal, pager, "Single page"
link,
and back-to-top button all drop out; tab groups expand so every panel is
visible, each labeled with its tab's name.

For printing a whole topic or book, use its
[`/print` view](~trail/building/the-output-surface) rather than printing pages
one at a time.

## Without JavaScript

The pages are static HTML and read fine with scripts disabled. What is lost is
the interactive layer: search, the theme toggle, text size, copy buttons, tab
switching, the drawer, scroll-spying, and diagram rendering. Content,
navigation links, the sidebar tree, and every `~link` work regardless.

If search cannot load its index or a result, the dialog reports the failure.
Enter a query again to retry. Results from an older query do not replace
the active query or command list.

If the browser refuses to copy a citation, its link title reports the
failure and contains the citation text. The link does not show copied
feedback unless the clipboard write succeeds.
