---
title: Writing Code Blocks
type: how-to
description: Fenced code blocks are highlighted at build time into themed classes, wrapped with a copy button, and labeled with their language. Which languages the bundled highlighter knows, and what happens to the ones it does not.
related:
  - trail/writing/articles-and-frontmatter
  - trail/writing/admonitions-tabs-and-diagrams
  - trail/building/theming
  - trail/building/the-reading-experience
---

<span id="highlighting-is-classes-not-colours"></span>

Fenced code blocks work as they do in any markdown, with an info string naming
the language:

````markdown
```rust
fn main() {
    println!("hello");
}
```
````

Trail highlights that at build time and wraps it with a copy button:

```html
<div class="code-block" data-language="rust">
  <button class="copy-code" type="button" data-copy-code>…</button>
  <pre><code><span class="hl-storage hl-type hl-function">fn</span> …</code></pre>
</div>
```

## Complete the task

1. Choose a language name for the block, or `text` for unhighlighted output.

2. Write a fenced code block in an article using that language name.

3. Put commands in a shell block without a prompt, and show their output in a
   separate `text` block.

4. Run `trail build --strict` from the site root and check the highlighting
   and copy button.

## Highlighting is classes, not colors

Highlighting happens once, when the site is built — there is no highlighter
shipped to the browser and no flash of unstyled code.

What is emitted is **class names**, not inline colors: `hl-keyword`,
`hl-string`, `hl-comment` and so on, prefixed so they cannot collide with the
theme's own classes. Those classes resolve through the same CSS custom
properties as everything else on the page, which is why code follows the
reader's light or dark choice instead of freezing one palette into the markup.
[Theming](~trail/building/theming) covers the tokens involved.

## The copy button

Every code block gets one. It sits in the top-right corner, appears on hover
(and is always visible on touch devices), copies the block's text to the
clipboard, and briefly says so.

Nothing is stripped from what it copies, so a `$` prompt in a shell example is
copied along with the command. Write shell examples without prompts if you
would rather they paste cleanly.

Code blocks returned by plugins preserve whitespace-only lines, including
spaces, tabs and form feeds. This also applies inside lists and blockquotes,
so live file excerpts retain their content when Trail inserts the plugin
output.

## Which languages are highlighted

Trail bundles [syntect](https://github.com/trishume/syntect) as its
highlighter, with the extended syntax collection from
[`two-face`](https://codeberg.org/CosmicHarper/two-face) — the same
definitions [bat](https://github.com/sharkdp/bat) ships, a little over two
hundred of them. The info string is matched against a syntax's name or one of
its file extensions, case-insensitively, so both `rust` and `rs` work.

Everything you are likely to reach for is covered:

```text
bash / sh / fish / zsh   asm         awk          cmd / bat          c / c++ / c#
clojure    cmake         coffeescript   crystal    css / scss / sass / less / stylus
d          dart          diff         dockerfile  dotenv     elixir   elm
erlang     f#            f90          gd          gitconfig / gitignore
glsl       go / gomod    graphql      dot         groovy     haskell  hcl / terraform
html       http          idris        ini         java       js       jq
json / jsonnet           julia        kotlin      latex / bibtex      lean
lisp       llvm          lua          make        markdown   matlab   nginx
nim        nix           objective-c  ocaml       odin       orgmode  pascal
perl       php           protobuf     puppet      purescript python   qml
r          racket        rego         robot       ruby / haml / slim   rust
scala      solidity      sql          svelte      swift      systemverilog / verilog / vhdl
tcl        toml          typescript / tsx        typst       vim      vue
wgsl       xml / svg     yaml         zig
```

…alongside the config and system formats that turn up in documentation:
`crontab`, `fstab`, `passwd`, `resolv`, `syslog`, `ssh_config`, `known_hosts`,
`.htaccess`, `requirements.txt`, `csv` / `tsv`, `manpage`, `strace`, `email`.

**Still absent:** `powershell`, and the shell-session forms (`console`,
`shell-session`). A `console` block renders as plain text — which is usually
what you want anyway, since a `$` prompt is not shell syntax.

An info string the set does not recognize is **not** an error and does not
fail the build: the block renders as plain escaped code, with the same
wrapper, the same copy button, and its language still recorded on it as
`data-language`.

The exception is an info string that opens with `{name}`. That is a
[plugin block](~trail/writing/plugin-blocks), expanded by a command the site
configures, and naming a plugin that is not configured _is_ an error.

Two languages are not code at all: `mermaid` blocks become
[diagrams](~trail/writing/admonitions-tabs-and-diagrams), and `math` blocks
become [equations](~trail/writing/equations).

The definitions' licenses ship with every build at
`/assets/LICENSE-Syntaxes.txt`.

## Blocks with no language

A fence with no info string renders as escaped plain text, with the wrapper
and copy button intact. Use one for output, trees, and anything that is not a
language:

````markdown
```
built 1227 pages (8 collections) → ./dist
```
````

Across this site the convention is `text` for that. The highlighter does not
recognize `text` as a language, so it renders identically to a bare fence
while reading more explicitly in the source.

Indented code blocks (four spaces) are supported too, and are treated as
having no language.

## Inline code

Inline `` `code` `` is ordinary markdown and is not highlighted. It is also
excluded from [inline reference](~trail/writing/inline-references) matching: a
claimed phrase inside backticks stays plain text, so writing `` `PCDS GUID` ``
when you mean the literal string does not produce a link.

## In the other views

Code blocks survive into the
[markdown mirrors and `/print` bundles](~trail/building/the-output-surface) as
ordinary fenced blocks with their info strings intact — the mirrors reproduce
your source, not the rendered HTML. Plugin blocks are the exception: they
appear expanded.
