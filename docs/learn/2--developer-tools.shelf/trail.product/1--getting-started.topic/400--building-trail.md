---
title: Building Trail
type: how-to
description: Build, test, format, and install Trail with pekit.
related:
  - trail/getting-started/quick-start
  - trail/reference/cli
---

Use these commands from the Trail source directory. [pekit](https://learn.peios.org/pekit/)
builds and tests Trail. Cargo manifests and the Cargo lockfile still define
the Rust dependencies.

## Prepare the host

The supported build host is x86_64 Linux. Install pekit, with bubblewrap
0.11 or later, and Git. The first build needs network access to download
the pinned tools and dependencies. pekit downloads Rust 1.98.1, the C
toolchain, Node.js 22, Python, and the crates `Cargo.lock` records, and
checks each against its pinned hash. `pekit.lock` pins the libraries that
provide them.

## Build and inspect the binary

1. Build the executable:

   ```sh
   pekit build trail
   ```

   pekit prints where the executable is, under `pekit-out/`.

2. Check the version and help:

   ```sh
   "$(tools/pekit-output trail)" --version
   "$(tools/pekit-output trail)" --help
   ```

The executable contains its templates, fonts, scripts, and search assets.
Copying the binary does not require copying a theme directory. pekit builds
each crate as its own step, in a sandbox that holds only its declared inputs.
A rebuild reuses each step whose inputs did not change. Embedded theme and
vendored Pagefind files are inputs, so changing them rebuilds the steps that
read them.

The executable links glibc dynamically. The `glibc` test fails if it needs a
glibc newer than 2.39, the version on GitHub's Ubuntu 24.04 runners.

## Run checks

The style and manual checks also require Rust 1.98.1 with rustfmt from
rustup, Python 3.11 or later, ShellCheck, and groff. The formatter uses the
host Rust toolchain that `rust-toolchain.toml` selects. Prettier and Node.js
are the versions pekit pins.

1. Install the matching Rust formatting tools if they are absent:

   ```sh
   rustup toolchain install 1.98.1 --profile minimal --component rustfmt
   ```

2. Run the complete check set:

   ```sh
   tools/check
   ```

3. Confirm that all checks pass. The tests are Rust unit and subprocess
   tests, Clippy with warnings denied, the theme's browser scripts, a strict
   documentation build, and the glibc check. After them, `tools/check` checks
   formatting, source hygiene, and manual rendering.

For a narrower check, name the tests:

```sh
pekit test units clippy
pekit test theme docs glibc
tools/style
tools/check-man
```

The Rust tests run with the pinned Node.js, Python, busybox, and a Git built
from pinned source. A test whose inputs did not change is not run again.
GitHub runs `tools/check --strict` on every branch and pull request.

## Format maintained sources

```sh
tools/style --write
```

This command edits the checkout. It formats first-party Rust, JavaScript,
CSS, embedded template scripts, and workflow files. It preserves imported
vendor layout except for Trail's maintained Pagefind patch files. Run
`tools/check` afterward to verify the result.

## Install the executable and manuals

Run this command to install under `$HOME/.local`:

```sh
tools/install
```

The executable goes to `bin/trail`, and manuals go to `share/man/man1` and
`share/man/man5` beneath that prefix. Add the prefix's `bin` directory to
your `PATH` if needed. Check the installed executable with
`$HOME/.local/bin/trail --version`.

To select a different prefix, pass an absolute path:

```sh
tools/install /absolute/installation/prefix
```

## Manage caches and dependency updates

Removing `pekit-out/` removes the build outputs in the checkout. pekit keeps
its store of steps outside the checkout, under its cache directory. Neither
holds the documentation cache described in
[Build Cache](~trail/building/build-cache) or installed files.

Keep `Cargo.toml` and `Cargo.lock` together when updating Rust dependencies,
and update the lockfile with Cargo. pekit reads the lockfile and never
changes it. After a change to `package-lock.json`, run
`pekit run pin_packages` to write `npm.pins` again, and review both together.
Run `pekit lock` to move the pinned libraries to their latest commits.
