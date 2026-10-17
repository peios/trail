---
title: Images
type: how-to
description: Images live beside the articles that use them and are referenced by relative path. Dimensions, captions and figures, what happens to an unreferenced file, and which formats trail recognises.
related:
  - trail/writing/articles-and-frontmatter
  - trail/structuring-a-site/topics-and-folders
  - trail/writing/admonitions-tabs-and-diagrams
  - trail/reference/directory-names
---

Images live **beside the articles that use them**. There is no assets directory to maintain and no separate URL scheme to remember: drop the file in the same folder as the `.md`, and reference it by name.

```text
2--logon.topic/
├── 100--overview.md
└── logon-flow.svg
```

```markdown
![The logon sequence](logon-flow.svg)
```

The image publishes at the URL of the directory it was found in, keeping its own file name — here `/peios/logon/logon-flow.svg`. Move the folder and the image moves with it.

## Referencing

Destinations resolve **relative to the article's own `.md` file**, exactly as an editor's markdown preview resolves them. Relative paths may climb out of a subfolder with `..`, so a diagram shared by a folder's worth of articles can sit one level up:

```text
2--concepts.topic/
├── shared-diagram.svg
└── 300--replication/
    ├── 100--topology.md          ![](../shared-diagram.svg)
    └── 200--conflicts.md         ![](../shared-diagram.svg)
```

Three kinds of destination are left exactly as written and never checked: absolute URLs (`https://…`), root-relative paths (`/…`) and `data:` URIs.

A `~` reference in an image destination is a build error. `~` names pages, not files.

## Where images may sit

Anywhere articles may sit: a topic, a topic subfolder, a book, or a book chapter. Not in a product, anthology or shelf directory — those hold only `trail.toml` and subdirectories.

The file name must be a valid slug plus a **lowercase** extension: `logon-flow.svg`, not `Logon Flow.SVG`. Recognised extensions are:

```text
png   jpg   jpeg   gif   svg   webp   avif
```

Anything else in an article directory is an "unexpected file" error — which is the point. A `.pdf` or a `.xlsx` sitting in a content folder is almost always a mistake, and trail would rather say so than quietly ignore it.

## Dimensions

Trail reads each raster image's header at build time and emits real `width` and `height` attributes:

```html
<img src="/peios/logon/screenshot.png" alt="The logon prompt" width="1280" height="720">
```

The browser can then reserve the right space before the file arrives, so text does not jump around as images load. SVGs get no dimensions — they scale — and neither does any file whose header cannot be read.

## Captions and figures

An image with a **title** — the quoted string after the destination — that sits alone in its own paragraph becomes a `<figure>` with a `<figcaption>`:

```markdown
![Three services exchanging tokens](logon-flow.svg "How a logon token reaches the session service")
```

```html
<figure>
  <img src="/peios/logon/logon-flow.svg" alt="Three services exchanging tokens">
  <figcaption>How a logon token reaches the session service</figcaption>
</figure>
```

The two strings do different jobs and should say different things. The **alt text** describes the image for someone who cannot see it. The **caption** is prose everyone reads, and can say what the diagram is *for*.

An image without a title, or one sharing its paragraph with other text, renders as an ordinary inline `<img>`.

## Unreferenced images

Only images some article actually references are copied into the output. One that nothing references prints a warning and is left out:

```text
warning: image '/…/2--logon.topic/old-diagram.png' is not referenced by any article and was not published
```

A warning rather than an error, deliberately: adding the file before the article that will use it is a normal way to work. But a warning that stays warning is a file to delete.

A referenced image that does not exist is the other way round — a broken link, fatal unless `--allow-dangling-links` downgrades it:

```text
in article '/peios/logon/overview': image 'logon-flow.svg' not found (resolved relative to the article's folder)
```

## In the other views

Images work in the [markdown mirrors and `/print` bundles](~trail/building/the-output-surface) too: since those serve an article's body away from its own folder, relative destinations are rewritten to the image's published URL.

## Diagrams

For flowcharts, sequence diagrams and the like, consider a [mermaid code block](~trail/writing/admonitions-tabs-and-diagrams) instead of an image file. Mermaid diagrams are text — diffable, editable, and they follow the reader's light or dark theme, which a checked-in PNG cannot.
