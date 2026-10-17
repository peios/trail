# Trail coding conventions

Trail follows the common coding rules and applicable Rust, JavaScript,
shell, and other-format profiles in
[PSC](https://learn.peios.org/psc/). Public documentation follows Core
Style, Learn, Man Pages, and Command-Line Help and Errors.

Run `npm ci --ignore-scripts`, then `tools/check` from a checkout. The check
requires the selected Rust toolchain, Node.js 22, Python 3.11 or later,
ShellCheck, and groff. `CARGO_TARGET_DIR` can select a reusable build directory.

Use `cargo fmt --package trail` for first-party Rust and `npm run format`
for JavaScript, embedded scripts, CSS, JSON, and workflow files. Rust's
toolchain and formatter edition are selected in the repository; Prettier's
exact version and dependency integrity are locked. Do not format imported
dependencies with `cargo fmt --all`.

## Scoped format decisions

- Imported Pagefind code, bundled Mermaid, fonts, and their licenses keep
  upstream spelling, naming, and layout. This preserves provenance and
  reviewable updates. Trail's Pagefind changes are retained in
  `vendor/pagefind/batch-api.patch`; its five modified source files receive
  the Rust formatting check. Regenerate the patch against pristine 1.5.2
  after changing them. Do not include unrelated vendor formatting.
- Jinja markup retains its layout because whitespace controls rendered
  output. Inline JavaScript still receives the same formatting and braces
  as standalone JavaScript. The formatter substitutes the one unquoted
  `build_config` expression while parsing, then restores it.
- Markdown code examples, terminal output, and protocol/configuration
  literals preserve their required syntax. Wrap prose at 78 columns. Keep
  admonition markers on their own blockquote line; generic Markdown
  formatters can merge that line and change Trail's interpretation.
- Rustfmt governs Rust layout. Long diagnostics, URLs, literal fixtures,
  and format-specific tables can exceed the 100-column review target when
  wrapping would make them harder to verify. Function length and nesting
  thresholds prompt review; they are not arbitrary rejection limits.
- The large Markdown event walker and site traversal routines retain their
  cohesive parser state. Review their state transitions and error paths
  when changing them rather than splitting them only to meet a line count.

## Interfaces and ownership

Preserve configuration keys, plugin JSON fields, published paths, and
serialized cache shapes during naming or formatting changes. Public command
and file interfaces belong in `man/`; Learn provides tasks and explanation.
Change help, manual, and observable CLI tests together.

Cache storage is disposable: failed cache reads/writes cause recomputation.
Mutexes protect entry maps; plugin execution and dependency hashing happen
outside shared map locks. Cache sessions own per-build state and pass shared
handles explicitly to worker threads. The development-server watcher belongs
to the CLI process; browser listeners belong to the page lifetime.

## Work and releases

Track work in Acta and use `<type>/<task-id>-<description>` branches.
Use imperative Conventional Commits, a concise subject, and a `Refs:` footer
with the task identifier. Keep unrelated work separate and validate a slice
before committing it. Preserve pre-existing untracked work.

The `latest` download is the existing continuous-build channel consumed by
documentation builds. It is not a numbered component release. Versioned
release tags and artifacts are immutable; do not use the moving channel as
the provenance of a qualified release. OS monthly-composition rules apply
when a component participates in that composition.

Keep adoption decisions and validation evidence in Acta, outside the public
documentation tree.
