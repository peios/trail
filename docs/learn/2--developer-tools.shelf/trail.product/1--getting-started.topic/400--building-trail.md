---
title: Building Trail
type: how-to
description: Build, test, format, and install Trail with the pinned Bazel tools.
related:
  - trail/getting-started/quick-start
  - trail/reference/cli
---

Use these commands from the Trail source directory. Bazel replaces the pekit
build, test, install, and clean entry points. Cargo manifests and the Cargo
lockfile still define the Rust dependencies.

## Prepare the host

The supported Bazel build host is x86_64 Linux. Install Bash, curl,
`sha256sum`, and a C/C++ compiler and linker. The first build needs network
access to download the pinned tools and dependencies.

`./tools/bazel` verifies and runs Bazel 8.4.2. It stores the executable under
`${XDG_CACHE_HOME:-$HOME/.cache}/peios/bazel/`. Bazel downloads Rust 1.96.0
and resolves the dependencies recorded in `Cargo.lock`. An installed Bazel
8.4.2 or Bazelisk that reads `.bazelversion` also works.

## Build and inspect the binary

1. Build the optimized executable:

   ```sh
   ./tools/bazel build //:trail
   ```

2. Check the version and help:

   ```sh
   bazel-bin/trail --version
   bazel-bin/trail --help
   ```

The executable contains its templates, fonts, scripts, and search assets.
Copying the binary does not require copying a theme directory. Repeating the
build reuses unchanged Bazel outputs. Embedded theme and vendored Pagefind
files are build inputs, so changing them invalidates the affected actions.

## Run checks

Checks also require Rust 1.96.0 with rustfmt, Node.js 22, Python 3.11 or
later, ShellCheck, and groff. The formatter uses the host Rust toolchain;
the native build uses the compiler downloaded by Bazel. Prettier and
buildifier are downloaded with pinned versions and integrity checks.

1. Install the matching Rust formatting tools if they are absent:

   ```sh
   rustup toolchain install 1.96.0 --profile minimal --component rustfmt
   ```

2. Run the complete check set:

   ```sh
   tools/check
   ```

3. Confirm that all tests pass. This includes Rust unit and subprocess tests,
   browser scripts, formatting, source hygiene, manual rendering, and a
   strict documentation build. The command also builds the Clippy target,
   which rejects warnings in first-party Rust.

For a narrower check, select its target:

```sh
./tools/bazel test //:unit-tests //:cli-test //:build_cache-test
./tools/bazel test //:browser-tests //:style //:manual-tests //:docs-test
./tools/bazel build //:clippy
```

Local host-tool checks rerun rather than trusting cached results when the
tools can change outside Bazel. Build actions use declared inputs, but the
host C/C++ toolchain and the local style tools are not fully hermetic.

## Format maintained sources

```sh
./tools/bazel run //:format
```

This command edits the checkout. It formats first-party Rust, JavaScript,
CSS, embedded template scripts, workflow files, and Starlark. It preserves
imported vendor layout except for Trail's maintained Pagefind patch files.
Run `tools/check` afterward to verify the result.

## Install the executable and manuals

Run this command to install under `$HOME/.local`:

```sh
./tools/bazel run //:install
```

The executable goes to `bin/trail`, and manuals go to `share/man/man1` and
`share/man/man5` beneath that prefix. Add the prefix's `bin` directory to
your `PATH` if needed. Check the installed executable with
`$HOME/.local/bin/trail --version`.

To select a different prefix, pass an absolute path:

```sh
./tools/bazel run //:install -- /absolute/installation/prefix
```

## Manage caches and dependency updates

`./tools/bazel clean` removes Bazel build outputs. It does not remove the
documentation cache described in [Build Cache](~trail/building/build-cache),
previous Cargo outputs, installed files, or old pekit output directories.

For a shared local action cache, pass a disk-cache path:

```sh
./tools/bazel build --disk_cache=/absolute/cache/path //:trail
```

Keep `Cargo.toml` and `Cargo.lock` together when updating Rust dependencies.
Normal Bazel builds preserve the Cargo lockfile. After an intentional update,
run `CARGO_BAZEL_REPIN=1 ./tools/bazel build //:trail`, then rerun the checks.

`MODULE.bazel.lock` is generated locally because crate_universe records the
absolute path of the maintained Pagefind dependency. Its Pagefind
`BUILD.bazel` is also generated and ignored. Do not edit or copy these files
between checkouts. Bazel module versions and tool download hashes are pinned
in the checked-in configuration; this does not establish byte-identical
output across different host C/C++ toolchains.
