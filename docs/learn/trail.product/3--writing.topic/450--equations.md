---
title: Equations
type: how-to
description: TeX equations, on their own in a math fence or inside a sentence between dollar signs, rendered to MathML when the site is built. Why plain dollar signs stay text, and what happens to TeX that does not parse.
related:
  - trail/writing/code-blocks
  - trail/writing/admonitions-tabs-and-diagrams
  - trail/building/the-output-surface
---

Equations are written in TeX, in one of two forms, both of which GitHub renders the same way.

An equation on its own goes in a code block tagged `math`:

````markdown
```math
L = \left( \frac{L' + 0.099}{1.099} \right)^{\frac{1}{0.45}}
```
````

which renders as:

```math
L = \left( \frac{L' + 0.099}{1.099} \right)^{\frac{1}{0.45}}
```

An equation inside a sentence is a code span with a dollar sign hard against each side:

```markdown
The average decays by a factor of $`\frac{2^N - 1}{2^N}`$ each sample.
```

which renders as: the average decays by a factor of $`\frac{2^N - 1}{2^N}`$ each sample.

Each `math` block holds one equation. Several lines are one equation only inside an environment that sets them out, such as `align*` with its `&` and `\\`:

````markdown
```math
\begin{align*}
B' - Yc' \le 0:& \\
&Cbc = (B' - Yc') / 1.9404
\end{align*}
```
````

## Plain dollar signs are text

`$x$` is **not** an equation. Documentation is full of dollar signs that are nothing of the kind: `$PATH and $HOME`, `costs $5 or $10`, `$dport_dev ($dport_dev`. Any two of those in a paragraph would pair up into a garbled equation, and no error would say so. The code-span form never happens by accident, and a dollar sign needs no escaping anywhere.

What sits inside a `math` block or the code span is passed to TeX exactly as written. Markdown escapes don't apply there, so `\_` stays `\_`.

## What it becomes

Trail renders each equation to [MathML](https://developer.mozilla.org/docs/Web/MathML), which browsers lay out natively, at build time:

- **No script and no font download.** An equation works in [print bundles](~trail/building/the-output-surface) and with JavaScript off.
- **The TeX stays with the equation.** It is attached as an annotation, so screen readers and copying can use it.
- **The Markdown mirrors keep the TeX as written.** Readers of the [mirror](~trail/building/the-output-surface) get the fence or code span, which they can render themselves.

An equation on its own is spaced like a paragraph. If it is wider than the column, it scrolls rather than squeezing.

## TeX that doesn't parse

An equation that fails to parse fails the build, as a broken link does, naming the equation and what is wrong with it:

```text
in article '/guide/colour': equation '\frac{1}' is not valid TeX: parsing error: expected a token
```

Trail renders with [pulldown-latex](https://github.com/Carlosted/pulldown-latex), which covers the commands of everyday mathematics: fractions, roots, sub- and superscripts, `\text`, `\left`/`\right`, Greek letters and relations, and the `align`, `matrix` and `cases` families of environments.
