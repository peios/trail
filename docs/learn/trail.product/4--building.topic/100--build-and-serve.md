---
title: Building and Serving
type: how-to
description: The two commands — trail build writes a static site into dist/, trail serve adds a watching dev server with live reload. What each flag does, what gets pruned, and how failures behave.
related:
  - trail/reference/cli
  - trail/building/the-output-surface
  - trail/writing/links-and-references
  - trail/getting-started/quick-start
---

<span id="allow-dangling-links"></span>
<span id="cbpath-path"></span>
<span id="deploying"></span>
<span id="out-dir"></span>
<span id="render-llms-full"></span>
<span id="strict"></span>
<span id="the-flags"></span>
<span id="the-output-directory-belongs-to-trail"></span>
<span id="trail-build"></span>
<span id="trail-serve"></span>
<span id="when-a-build-fails"></span>

Build a static site for publication or preview it while editing. You need
a Trail binary on your `PATH` and a source directory containing `trail.toml`.
For an empty directory, follow [Building Your First
Site](~trail/getting-started/quick-start).
`trail(1)` documents the complete command interface.

## Build the site

1. Change to the directory containing `trail.toml`.
2. Build with description checks:

   ```sh
   trail build --strict
   ```

3. Check that the command exits successfully and reports the output directory.
   The page count counts HTML pages, including print pages. Markdown mirrors,
   search files, and static assets are additional files.

Output defaults to `dist/`. Use `--out <directory>` to choose another
location. The output directory is excluded from source scanning.

## Own the output directory

A successful build removes files it did not produce. Put additional files
such as `CNAME` in the source tree and configure
[`passthrough`](~trail/building/configuring-the-site).
Do not keep manually managed files in `dist/`.

A failed build does not prune output. It can have written some files before
failing, so the directory can contain a mixture of old and changed pages.
Unchanged files retain their modification times.

## Preview changes

1. Start the development server from the source directory:

   ```sh
   trail serve
   ```

2. Open the address printed by the server. It defaults to
   `http://127.0.0.1:8724`.
3. Edit and save an article. Check that Trail reports a rebuild and the
   browser reloads the page.
4. Stop the server with **Ctrl+C**.

The server watches merged trees and declared plugin dependencies. It ignores
output paths and hidden files, and periodically reconciles source contents
to detect missed events. [Build Cache](~trail/building/build-cache) explains
reuse and invalidation.

A failed initial build or rebuild prints a warning and keeps the server
running. Fix the reported source problem and save again. Output is not
rolled back after a failure.

## Diagnose a failure

Trail reports errors on stderr with the invocation name and source context.
Usage errors also print a help pointer. Use `trail help build` to check
arguments, or `trail build --no-cache` to reproduce a build without cached
results.

Missing link targets fail builds by default. During a site reorganization,
`--allow-dangling-links` turns missing targets into warnings; ambiguous links
still fail. Resolve warnings before publishing.

`--strict` requires descriptions and warns about authored site-absolute
links. It does not enable link validation: links are validated on every
build. [Writing Links and References](~trail/writing/links-and-references)
explains the linking syntax.

## Publish other output forms

[The Output Surface](~trail/building/the-output-surface) describes Markdown
mirrors, print bundles, and discovery files. Use `--render-llms-full` when
you need additional `llms-full.txt` copies. Use `--cbpath <path>` to publish
[a cache-busting copy](~trail/building/the-cache-busting-copy).
