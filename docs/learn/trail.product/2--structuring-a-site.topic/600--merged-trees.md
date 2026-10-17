---
title: Merging Source Trees
type: how-to
description: Build one site from several directories of products, such as documentation kept in two repositories. How to list a tree with [[merge]], which settings come from which trail.toml, and how links, plugins, edit links, and serve behave across trees.
related:
  - trail/reference/trail-toml
  - trail/building/configuring-the-site
  - trail/writing/plugin-blocks
  - trail/building/build-and-serve
---

<span id="what-a-merged-tree-s-root-may-hold"></span>

A site doesn't have to live in one directory. When documentation is kept in
more than one repository, for example a kernel's reference beside its source
and the rest of the project's guides elsewhere, one site can **merge** the
other trees' products into its own.

The site you build is the **primary**. Its root `trail.toml` lists each tree
to merge:

```toml
sitename = "Peios Learn"
title = "Documentation for everything Peios"
description = "Concepts, guides, and reference for Peios."
footer = "Peios Learn: documentation for Peios software."

[[merge]]
path = "../kernel/docs"
```

`path` is relative to the primary's root. A merged tree is laid out exactly
like a site root: `*.product` directories and, optionally, a `trail.toml` of
its own. Each tree can still be built as a site by itself.

Like every table, `[[merge]]` must come after the file's plain keys.

## Complete the task

1. Choose the source trees to combine and ensure their product slugs do not
   conflict.

2. Add a `[[merge]]` entry to the destination site configuration for each
   tree.

3. Run `trail build --strict` from the destination root and resolve
   configuration or reference errors.

4. Check that all expected products appear in navigation and that links
   between trees resolve.

## What joins the site

Every product in a merged tree joins the site as if it were in the primary's
root. It appears on the front page and in the products menu, sorts by
`featured` like any other product, and is in the search index.

Because `~` references name a product by its slug, a page in one tree can link
to a page in another: `~kernel/process/submitting-patches` works from any
tree. Two trees can't both have a product with the same slug; that's a build
error naming both trees.

## Which trail.toml decides what

The primary's `trail.toml` decides everything about the site: its name, title,
description, footer, URL, navigation, featured products, accent colors,
favicon, custom CSS, `head_html`, and theming. The same keys in a merged
tree's `trail.toml` are ignored, and so is its own `[[merge]]` list.

A merged tree's `trail.toml` still matters for the settings that belong to its
files:

| Key | In a merged tree |
| ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `edit_url` | Edit links on the tree's pages use the tree's own template, with `{path}` relative to the tree's root. A tree without `edit_url` gets no edit links; it doesn't borrow the primary's, which would point at the wrong repository. |
| `passthrough` | Entries are copied from the tree's root. Where two trees pass the same path through, the primary's copy wins, then the first tree listed. |
| `attachments` | Sources resolve from the tree's root. IDs must be unique across all trees. |
| `plugins` | The tree's plugins are available to every page, and each runs in the tree that configures it. See [Plugins](#plugins). |

A merged tree doesn't need a `trail.toml` at all. If it has one, the file must
be valid, with the same required keys as any site root, so that the tree still
builds by itself.

## What a merged tree's root can hold

A merged tree's root is checked like a site root: `trail.toml`, `*.product`
directories, dot-entries, and whatever its own `trail.toml` names. A `dist/`
directory is tolerated too, so a tree that has been built by itself still
merges.

A merged tree can sit inside the primary's root. Naming it in `[[merge]]` is
what makes its directory legal there, the same way `favicon` and `passthrough`
entries are.

## Plugins

[Plugins](~trail/writing/plugin-blocks) from every tree are combined. A plugin
runs in the tree whose `trail.toml` configures it:

- Its program path is relative to that tree's root;
- Its working directory is that tree's root, and so is the `root` its request
  carries;
- Each block's `file` is relative to the root of the tree the page is in, so a
  plugin sees the same paths it would if that tree were built by itself.

A plugin configured in a merged tree as
`command = ["tools/kernel-doc", "--srctree", ".."]` therefore works unchanged
when the tree is merged.

Where two trees configure a plugin with the same name, the primary's wins,
then the first tree listed.

## Building a tree by itself

A merged tree's pages can link into the other trees' products. Built by
itself, such a tree has broken links, and `trail build --strict` fails. That's
expected: build it without `--strict`, or with `--allow-dangling-links` to get
past the broken links entirely.

## Serving

[`trail serve`](~trail/building/build-and-serve) watches every merged tree as
well as the primary, with everything beneath it except dot-entries and the
tree's own `dist/`. Editing a page in any tree rebuilds the site.
