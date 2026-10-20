---
title: Attachments
type: how-to
description: Publish files and reproducible source bundles with checked links and download metadata.
related:
  - trail/building/configuring-the-site
  - trail/writing/links-and-references
---

Declare downloads in the site root's `trail.toml`. Trail can copy a file or
package a directory as a `.tar.gz` archive. It validates the source and
references, publishes the download, and records its size and SHA-256 digest.

## Declare a source bundle

Keep the source in a dedicated directory outside the product tree:

```text
docs/
├── trail.toml
├── download-sources/
│   └── tools/
│       ├── README.md
│       └── scripts/
└── example.product/
```

Add a declaration to the root configuration:

```toml
[[attachments]]
id = "example-tools"
title = "Example Tools"
description = "Configuration files and setup scripts."
source = "download-sources/tools"
filename = "example-tools.tar.gz"
format = "tar.gz"
exclude = [".venv", "node_modules", ".cache", "results"]
```

`source` is relative to the declaring tree's root. The declaration makes its
top-level source directory legal there without publishing the loose files.
Each merged tree can declare its own attachments. IDs must be unique across
all trees.

`exclude` lists exact paths relative to the bundle directory. A directory
entry excludes its whole subtree; entries are not glob patterns. All other
files, including hidden files, enter the archive. Keep generated environments
and local output outside the bundle or list them explicitly.

Declared attachment sources are data, so `trail lint` does not treat their
Markdown files as documentation articles. Check bundled code and README files
with the bundle's own tools.

Trail rejects symbolic links, special files, paths outside the source tree,
and sources that overlap the build output. It never executes bundled code.

## Publish a single file

Use `format = "file"`, the default, for a PDF, configuration file, or archive
produced by another tool:

```toml
[[attachments]]
id = "example-config"
title = "Example Configuration"
description = "A starting configuration for the example service."
source = "download-sources/example.toml"
filename = "example.toml"
```

The published bytes match the source. File attachments do not accept
exclusions.

## Link to downloads and metadata

Use the exact ID in an attachment reference:

```markdown
[Download the tools](~attachments/example-tools)
[Download metadata](~attachments/example-tools/metadata)
```

The first reference resolves to
`/attachments/example-tools/example-tools.tar.gz`. The second resolves to
`/attachments/example-tools/metadata.json`. Missing references are link
errors; `--allow-dangling-links` can downgrade them like other unresolved
references. Valid attachment links produce no `--strict` advisory warning.

Attachment links retain file URLs in Markdown exports. Print links use the
configured site URL, and cache-busting copies keep their own download URLs.

The `attachments` product slug and passthrough output directory are reserved
when a site declares attachments. IDs and filenames start with an ASCII
letter or digit and contain only ASCII letters, digits, dots, underscores,
and hyphens. The ID `index.json` and filename `metadata.json` are reserved.

## Inspect download metadata

Each metadata file contains `schema_version` (1), `id`, `title`,
`description`, `filename`, `url`, `metadata_url`, `format`, `media_type`,
`size_bytes`, and `sha256`. The digest covers the published bytes, including
compression for archives. `/attachments/index.json` lists the same records
in ID order. These records omit local source paths.

Generated archives use `application/gzip`; copied files use
`application/octet-stream` in the metadata. The hosting server controls its
HTTP content type and download headers. Trail does not configure that server.

## Rebuild bundles

Archive entries are sorted and placed under one directory named after the
attachment ID. Trail preserves file contents and whether Unix files are
executable; other permission bits, ownership, and timestamps are normalized.
Directories and executable files use mode `0755`; other files use `0644`.
Empty directories are preserved. Identical inputs with the same Trail build
produce identical archives, independent of checkout location or file dates.

Content and executable-mode changes invalidate the archive cache. Unchanged
downloads retain their output modification times. Removed declarations prune
their downloads and metadata on the next successful build. `trail serve`
also detects changes to declared hidden inputs and executable modes.
