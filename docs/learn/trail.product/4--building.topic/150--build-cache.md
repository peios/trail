---
title: Build Cache
type: concept
description: How Trail reuses build work, checks dependencies, and rebuilds without cached results.
related:
  - trail/building/build-and-serve
  - trail/writing/plugin-blocks
---

Trail reuses unchanged build work in `build` and `serve`. The first build
fills a cache. Later builds reuse plugin output, rendered Markdown, source
mirrors, and search files when their inputs match.

## Cache Location

The persistent cache lives in `.trail-cache/` under the site root. It is
separate from `dist/`, is not published, and is ignored by the file watcher.
Add `.trail-cache/` to your source repository's ignore file.

Cache entries belong to the Trail executable that created them. Installing
a different executable starts a separate cache. You can remove
`.trail-cache/` to reclaim space; Trail recreates needed entries. Successful
builds discard entries from their executable's cache that they did not use.

A missing, damaged, or unwritable cache causes Trail to compute the result
again. A cache failure does not prevent a build.

## Rebuild Without Cached Results

Pass `--no-cache` to bypass cache reads and writes:

```console
$ trail build --strict --no-cache
$ trail serve --no-cache
```

This flag applies to every rebuild in that `serve` session. It leaves
existing cache files in place. Output files whose bytes are unchanged
retain their modification times.

## What Invalidates Cached Work

Trail checks file contents. It does not rely on modification times alone.
Changes to link targets, anchors, inline references, image dimensions, or
rendering options invalidate the Markdown results that depend on them.
Page chrome and navigation are rendered from the loaded site on each build.
Print bundles retain their own anchor prefixes and link destinations.

Search files are reused only when the ordered set of indexed URLs and
rendered HTML is unchanged. Adding, editing, renaming, or deleting an
indexed page rebuilds the search index. Deleted pages and unused images
are still removed from the output after a successful build.

Plugins must declare every file they read in `depends_on`, including
imported helpers. A directory dependency tracks the names beneath it;
also list the files whose contents affect the result. Trail checks those
dependencies, the configured command, resolved executable, request inputs,
and environment before reusing a successful response. Plugin failures are
not cached. Plugin diagnostics are replayed on cache hits; moving a block
that produced diagnostics reruns its plugin to refresh source locations.

A plugin that depends on time, network state, or an undeclared input must
be run with `--no-cache` when that input changes. See
[Plugin Blocks](~trail/writing/plugin-blocks) for the dependency contract.

## Reuse Within a Build

Trail also shares parsed Markdown and highlighted code across article and
print passes. These memory caches have limits: 256 MiB of accounted parsed
events and 64 MiB of highlighted output per build thread. Search parsing
uses up to eight workers, limited by the available parallelism.

The cache does not remove build features. Strict validation, diagnostics,
Markdown mirrors, print bundles, search, and the cache-busting copy remain
part of the build.
