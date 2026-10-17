---
title: Build Cache
type: concept
description: How Trail reuses build work, checks dependencies, and rebuilds without cached results.
related:
  - trail/building/build-and-serve
  - trail/writing/plugin-blocks
---

Trail reuses unchanged build work in `build` and `serve`. The first build
fills a cache. Later builds reuse plugin output, heading and anchor data,
rendered Markdown, complete pages, print bundles, source mirrors, and
search files when their inputs match.

## Cache location

The persistent cache lives in `.trail-cache/` under the site root. It is
separate from `dist/`, is not published, and is ignored by the file watcher.
Add `.trail-cache/` to your source repository's ignore file.

Cache entries belong to the Trail executable that created them. Installing
a different executable starts a separate cache. You can remove
`.trail-cache/` to reclaim space; Trail recreates needed entries. Successful
builds discard entries from their executable's cache that they did not use.
Complete pages and print bundles trade disk space for less repeated work.
Their entries also retain the smaller cached results needed after an edit.

A missing, damaged, or unwritable cache causes Trail to compute the result
again. A cache failure does not prevent a build.

## Rebuild without cached results

Pass `--no-cache` to bypass cache reads and writes:

```console
$ trail build --strict --no-cache
$ trail serve --no-cache
```

This flag applies to every rebuild in that `serve` session. It leaves
existing cache files in place. Output files whose bytes are unchanged
retain their modification times.

## What invalidates cached work

Trail checks file contents. It does not rely on modification times alone.
Changes to link targets, anchors, inline references, image dimensions, or
rendering options invalidate the Markdown results that depend on them.
Link and section lookups track the answers they used, including missing
targets. An unrelated target change can reuse existing results; a newly
ambiguous or missing target still produces the appropriate diagnostic.
Page chrome, navigation, and build options are checked before reusing a
complete page. Print bundles retain their own anchor prefixes and link
destinations.

Search files are reused only when the ordered set of indexed URLs and
rendered HTML is unchanged. Adding, editing, renaming, or deleting an
indexed page rebuilds the search index, while unchanged fragments and
chunks reuse their compressed bytes. Unchanged pages also reuse their
parsed search data. Global index packing still runs after an edit.
Deleted pages and unused images are still
removed from the output after a successful build.

Plugins must declare every file they read in `depends_on`, including
imported helpers. A directory dependency tracks the names beneath it;
also list the files whose contents affect the result. Trail checks those
dependencies, the configured command, resolved executable, request inputs,
and environment before reusing a successful response. Plugin failures are
not cached. Plugin diagnostics are replayed on cache hits; moving a block
that produced diagnostics reruns its plugin to refresh source locations.
Independent plugins run in groups of up to four, limited by the available
parallelism. Declared `after` dependencies run first. Results and diagnostics
retain their dependency order, then name order, regardless of completion
order. Plugins must not depend on undeclared execution ordering.

A plugin that depends on time, network state, or an undeclared input must
be run with `--no-cache` when that input changes. See
[Plugin Blocks](~trail/writing/plugin-blocks) for the dependency contract.

## Reuse within a build

Article bodies render on up to four workers, limited by the available
parallelism. Small sites use one worker. Results and diagnostics retain
page order. Trail also shares parsed Markdown and highlighted code across
article and print passes. These memory caches have limits: 256 MiB of
accounted parsed
events and 64 MiB of highlighted output per build thread. Search parsing
and compression each use up to eight workers, limited by the available
parallelism. The site and its cache-busting copy share prepared link,
image, and inline-reference indexes.

## Development server rebuilds

`serve` keeps the authored site structure in memory. Changes to existing
articles refresh those articles. Added, removed, or renamed files and
configuration changes reload the structure. Plugin expansion, aliases,
reading times, links, and other derived results are checked on each build.
An uncertain watcher event also triggers a full reload.

The server compares source contents during rebuilds. It also checks for
missed changes every 30 seconds, including declared plugin dependencies.
These checks do not reload the browser when nothing changed. `--no-cache`
disables reuse of the retained site structure.

Output comparisons use bounded buffers. Unchanged files keep their
modification times; missing or modified output files are repaired during
the next build, even when their size and modification time are unchanged.

The cache does not remove build features. Strict validation, diagnostics,
Markdown mirrors, print bundles, search, and the cache-busting copy remain
part of the build.
