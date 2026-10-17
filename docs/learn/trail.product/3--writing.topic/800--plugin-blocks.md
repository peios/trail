---
title: Plugin blocks
type: how-to
description: Fenced blocks whose info string is {name} are expanded by an external command the site configures. The block syntax, the trail.toml table, the JSON protocol a plugin speaks, and how generated content behaves once it is on the page.
related:
  - trail/writing/code-blocks
  - trail/reference/trail-toml
  - trail/building/the-output-surface
  - trail/building/build-and-serve
---

Some content is not written but derived: API reference extracted from source comments, tables generated from a schema. A plugin block asks an external command to produce that content at build time, so the page and its source can never disagree.

````markdown
```{kernel-doc} drivers/base/core.c
:export:
```
````

The fence's info string is `{name}`, which is how [MyST](https://myst-parser.readthedocs.io/) spells a directive. Trail hands the block to the command the site configures under that name, and puts the markdown that comes back where the fence was.

## Writing a block

````text
```{name} argument
:option: value
:flag:

body
```
````

- **`{name}`** names a plugin configured in the root `trail.toml`. A `{name}` that is not configured is a **build error**, the same as a mistyped admonition marker. Otherwise it would render as a code block, and nobody notices a code block that should have been a page of reference.
- **The argument** is everything after `{name}` on the opening line. What it means is up to the plugin.
- **Options** are `:key: value` lines at the start of the block. A flag has no value. An indented line continues the previous option's value, and a trailing `\` on a piece is dropped, as in reStructuredText. Giving the same option twice is an error.
- **The body** is whatever follows the options, after one optional blank line.

A plugin block works anywhere a fenced code block does: inside list items and blockquotes, with backticks or tildes, at any fence length. To show the syntax on a page, as this one does, wrap it in a longer fence: the inner fence is then content, not a block. Indented code blocks are never plugin blocks.

An unclosed plugin fence is an error, because it would otherwise take the rest of the page as its body.

## Configuring a plugin

Plugins are declared in the root `trail.toml`, one table each:

```toml
[plugins.kernel-doc]
command = ["python3", "../tools/docs/trail-kernel-doc/trail-kernel-doc", "--srctree", ".."]
```

`command` is the program and its arguments, run directly, not through a shell. Two more keys, `anchors` and `after`, let a plugin [link to the rest of the site](#linking-to-the-rest-of-the-site). A program containing a `/` is a path relative to the site root and must exist. A bare name is looked up on `PATH`. The working directory is the site root.

A plugin configured in a [merged tree](~trail/structuring-a-site/merged-trees)'s `trail.toml` treats that tree's root as the site root throughout: its program path, working directory, `root` and `file` are all relative to the tree, exactly as when the tree is built alone. Its blocks may still appear on any page of the site.

Markdown can only *name* a configured plugin; it can never say what to run. Building a site runs only the commands its `trail.toml` lists.

Plugin names are lowercase letters, digits, `-` and `_`. The [`trail.toml` reference](~trail/reference/trail-toml) has the table's details.

## What a plugin receives and returns

Trail runs each plugin **once per build**, passing every one of its blocks on the site in a single request. A plugin with no blocks is not run at all. A plugin that cross-references its own output can therefore see everything at once: a function name in one block can link to the page where another block documents that function.

The request arrives as JSON on stdin:

```json
{
  "protocol": 1,
  "plugin": "kernel-doc",
  "root": "/home/me/kernel/docs",
  "blocks": [
    {
      "id": 0,
      "page": "/kernel/driver-api/core",
      "reference": "kernel/driver-api/core",
      "file": "kernel.product/2--driver-api.topic/1--core.md",
      "line": 14,
      "argument": "drivers/base/core.c",
      "options": { "export": "" },
      "body": ""
    }
  ]
}
```

| Field | |
|---|---|
| `protocol` | The protocol version, `1`. A plugin should refuse one it does not know. |
| `root` | The site root, absolute. |
| `page` | The URL path of the page the block is on. |
| `reference` | The same page as a `~` reference without the `~`. It is fully qualified, so `~` + `reference` always resolves, and a plugin can link back to a block's page with `~reference#anchor`. |
| `file`, `line` | The block's source file, relative to the root, and the line its opening fence is on. These are for the plugin's own messages. |
| `argument`, `options`, `body` | The block, parsed. |

The plugin answers on stdout:

```json
{
  "blocks": [
    { "id": 0, "markdown": "#### `device_add()`\n\n…" },
    { "id": 1, "error": "drivers/base/nope.c does not exist" }
  ],
  "depends_on": ["../drivers/base/core.c", "../include/linux/device.h"]
}
```

Every block id must be answered **exactly once**, each answer with **exactly one** of `markdown` or `error`. `depends_on` lists the files the output was derived from, relative to the root or absolute.

A plugin that scans a directory, rather than reading files it was told about, should list the directory as well as the files it read. A directory in `depends_on` stands for the set of files beneath it, so a file added there or removed counts as a change. Edits to those files count only through the files' own entries.

The plugin's stderr goes straight to the terminal, so it is the place for warnings.

## Linking to the rest of the site

A plugin sees only its own blocks. To link to something another page defines, such as a function documented by hand, it can ask for the site's anchors with `anchors = true` in its table. Its request then carries one more field: every page's explicit anchors, keyed by reference.

```json
"anchors": {
  "kernel/userspace/media/vidioc-querycap": ["c-v4l2_capability", "rst-vidioc-querycap"]
}
```

**Explicit anchors** are the ids written in raw HTML, `<a id="…">`, and citation anchors. Heading ids are left out: Trail derives them from the heading's words, so they change when a heading is reworded and are no basis for generated links.

The anchors come from the pages as written, before any plugin's output is added, so what one plugin generates never depends on another having run. To link to targets that another plugin generates, name it in `after`:

```toml
[plugins.kernel-include]
command = ["python3", "../tools/docs/trail-kernel-doc/trail-kernel-include", "--srctree", ".."]
anchors = true
after = ["kernel-doc"]
```

Trail then runs `kernel-doc` first and adds the anchors in its output to those `kernel-include` receives. Each plugin runs after every plugin its `after` names. Independent plugins can run together in bounded groups; their results and diagnostics retain dependency order, then name order. See [Build Cache](~trail/building/build-cache) for worker limits. `after` without `anchors`, a name that is not a configured plugin, and a cycle all fail the build.

The anchors are part of what a plugin's cached output depends on, so a new anchor on any page reruns the plugins that receive anchors.

## When something goes wrong

All of these fail the build:
- a plugin exits non-zero;
- it prints something that is not a valid response;
- it misses or repeats a block;
- it answers a block with `error`.

Block errors and malformed fences are collected across the whole site and reported together, each against its source line:

```text
2 plugin blocks failed:
  kernel.product/2--driver-api.topic/1--core.md:14: kernel-doc: drivers/base/nope.c does not exist
  kernel.product/2--driver-api.topic/3--bus.md:40: '{kernldoc}' fence names no configured plugin (configured: kernel-doc)
```

## Generated content is ordinary content

Expansion happens **when the site loads**, before anything reads a page. The markdown a plugin returns then behaves exactly as if it had been typed:

- its headings and raw HTML `id`s are anchors that links can target, and its own `~` links are checked, strictly under `--strict`;
- it counts toward reading time, unless the page's frontmatter sets `reading_minutes`;
- it is indexed for search and appears in `/print` bundles;
- `.link` aliases of the page carry it too.

That includes the [markdown mirrors](~trail/building/the-output-surface): a mirror shows the expanded content, not the fence. Readers of the mirror, people and machines alike, get the documentation rather than an instruction to go and generate it.

Expanded output is not expanded again. A `{name}` fence inside a plugin's output stays a plain code block.

To keep the output separate from the text around it, Trail sets it off with blank lines. Inside a blockquote or a list item, every output line gets the container's prefix.

## Rebuilds and caching

Trail keeps each plugin's last successful output. It reuses that output as long as all of these are unchanged:
- the plugin's blocks: arguments, options, bodies, and the pages they are on. A block's line number does not count, so editing prose above it costs nothing.
- everything in its `depends_on`. Files are compared by content, and directories by the names of the files beneath them, skipping names that start with a dot (such as editors' swap files). The plugin's own program also counts when it is configured as a path.
- for a plugin with `anchors = true`, the anchors it receives.

Under [`trail serve`](~trail/building/build-and-serve), a dependency outside the site root is watched as well, so editing a source file the plugin reads rebuilds the site. A change reruns the whole plugin, not just the affected blocks, because the plugin may cross-reference across all of them.

The cache lives in memory, so every `trail build` runs each plugin once.
