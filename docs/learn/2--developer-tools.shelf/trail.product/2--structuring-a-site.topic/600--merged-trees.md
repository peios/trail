---
title: Merging Source Trees
type: how-to
description: Combine documentation from several repositories while keeping
  shared navigation and each page's source, assets, and edit link.
related:
  - trail/structuring-a-site/anthologies-and-shelves
  - trail/structuring-a-site/link-aliases
  - trail/writing/plugin-blocks
  - trail/building/build-and-serve
---

A merged site combines documentation kept in separate repositories. Each
repository owns its pages, while matching containers provide one navigation
structure. Use this when a component's documentation belongs beside other
components' pages in the published site.

## Before you start

- Give each tree a valid Trail layout.
- Agree on shared container slugs, orders, and metadata.
- Give every article and alias one source owner.

## Combine the trees

1. Choose the site whose configuration controls the published result. This
   is the **primary** tree.

2. Add a `[[merge]]` table to its root `trail.toml`. Place tables after the
   file's plain keys. For example, this table includes a component's docs:

   ```toml
   [[merge]]
   path = "../../authd/docs/learn"
   ```

   The path is relative to the primary root, not the working directory.

3. Repeat the ancestor directories in each contributing tree. For example,
   two repositories can each contain this structure:

   ```text
   peios.antho/
     trail.toml
     100--accounts-and-sign-in.shelf/
       100--local-accounts.topic/
   ```

   Put each repository's articles inside its topic directory. Trail merges
   the anthology, shelf, and topic into one collection at `/peios`. The
   shelf adds a heading without adding a URL segment.

4. Give repeated containers identical metadata. A root anthology needs
   `title` and `description` in its `trail.toml`. For example:

   ```toml
   title = "Peios"
   description = "Using and developing the Peios operating system."
   ```

   Matching containers must also use the same type and order. Trail does
   not choose one repository's title, description, or reference claims over
   another's. See `trail-source(5)` and `trail.toml(5)` for the full rules.

5. Build the primary tree with strict validation:

   ```sh
   trail build --strict
   ```

## Check the result

Open the generated site. Confirm that shared containers appear once, pages
from every tree appear in reading order, and cross-tree links resolve.
Follow an edit link to check that it opens the page's owning repository.

A duplicate article, alias, or image destination is an error, even when both
files have identical contents. Remove the duplicate source or choose a
separate slug. Use a [link alias](~trail/structuring-a-site/link-aliases) when
one article belongs in two navigation locations.

Different items at the same level need different order numbers. Shelves
share their parent's URL space, so moving a topic into another shelf does
not make a duplicate topic slug distinct.

## Choose a root collection

Use `<slug>.product` for documentation that needs product branding. Use
`<slug>.antho` for a collection that needs a title and description without
product branding fields. Both appear in navigation and support `featured`.
A root anthology has no order prefix; its children use the usual prefixes.

Both root types can combine contributions. Matching nested anthologies,
shelves, topics, topic folders, books, and chapters combine recursively.
An anthology and a product with the same root slug conflict. A merge does
not convert one container type into another.

A tree's root can also hold
[root shelves](~trail/structuring-a-site/anthologies-and-shelves#use-a-shelf-at-the-site-root).
Shelves with the same slug combine and need the same order and title. A
collection contributed by several trees must sit on the same shelf in each,
or outside every shelf in each.

## Keep source ownership

Articles and images retain their original filesystem locations. Relative
image paths resolve beside the article, including in aliases. An edit link
uses the source tree's `edit_url` and a path relative to that tree. A tree
without `edit_url` does not inherit the primary's template.

Each tree keeps its declared attachments and passthrough files. Attachment
IDs are unique across the site. For overlapping passthrough paths, the
primary wins, followed by the first tree listed.

[Plugins](~trail/writing/plugin-blocks) run from the tree that configures
them. Each block's file path remains relative to the tree that owns the
article. For duplicate plugin names, the primary wins, followed by the first
tree listed.

The primary controls site presentation, navigation, and featured
collections. A merged tree's own `[[merge]]` entries are not followed. List
every required tree in the primary configuration.

## Build a component's docs separately

Add a valid root `trail.toml` to build a component's tree on its own. A tree
without that file can contribute content but cannot build independently.

Links to pages supplied by another tree require the combined build. For a
partial preview, build without `--strict`, or pass `--allow-dangling-links`.
Use the combined strict build to validate those references before publishing.

## Serve the combined site

Run `trail serve` from the primary root. It watches all merged trees and
rebuilds when their content changes. Dot-entries and each tree's `dist/`
directory are excluded from the content scan.
