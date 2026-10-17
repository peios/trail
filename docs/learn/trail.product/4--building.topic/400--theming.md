---
title: Theming
type: how-to
description: How a trail site is coloured — the site accent, per-product tints, the three-state light/dark system, the CSS custom properties everything resolves through, and the custom stylesheet escape hatch.
related:
  - trail/building/configuring-the-site
  - trail/structuring-a-site/products
  - trail/writing/code-blocks
  - trail/building/the-reading-experience
---

Trail ships one theme, compiled into the binary. There is no template directory to override and no theme to install. What you do control is colour — through configuration for the ordinary cases, and through CSS custom properties for everything else.

## The three colour settings

```toml
accent = "#3b82f6"          # the site's colour
accent_dark = "#7aa8ff"     # optional; derived from accent if absent
product_theming = true      # default
```

**`accent`** is the site's colour: links, focus rings, the wordmark's accented word, the hero glow, the front-page cards. Without it the theme's own default is used.

**`accent_dark`** is the dark-mode variant. Without it, trail derives one by mixing the accent 75% toward white — a light accent reads badly on a dark background, and most brand colours need lifting. Set it explicitly when the derived one is wrong. Setting it *without* `accent` is a build error.

Both must be `#rrggbb`.

**Each product also has a `color`**, and by default it displaces the site accent on every page belonging to that product: cards, the monogram tile, sidebar highlights, heading numbers, RFC-2119 keywords, focus rings. `product_theming = false` turns this off site-wide — every per-product tint collapses to the site accent, and products keep only their monogram and their card.

## Three-state light and dark

Every reader is in one of three states, and the header toggle cycles between them:

| State | |
|---|---|
| **System** | Follow the operating system's preference. The default. |
| **Light** | Pinned light, whatever the system says. |
| **Dark** | Pinned dark, whatever the system says. |

The choice is stored per reader in their browser and applied **before first paint**, so a reader who has chosen dark never gets a flash of white.

In CSS this is three blocks: `:root` carries the light tokens; `:root[data-theme="dark"]` overrides them when the toggle has pinned dark; and `@media (prefers-color-scheme: dark) :root:not([data-theme="light"])` overrides them for system-dark readers who have not pinned light.

> [!IMPORTANT]
> If you write custom CSS with colours in it, follow the same three-state pattern. Defining a colour only inside a `prefers-color-scheme` block means the header toggle cannot override it, and a reader who pins light on a dark machine gets your dark colour on trail's light page.

## The tokens

Everything resolves through CSS custom properties, so overriding a token restyles every place it is used.

| Token | |
|---|---|
| `--bg` | Page background. |
| `--surface` | Cards, menus, the sidebar — translucent over the background. |
| `--text` | Body text. |
| `--text-muted` | Secondary text: crumbs, meta lines, captions. |
| `--border`, `--border-strong` | Hairlines and emphasised edges. |
| `--accent` | The site accent, as resolved for the current theme. |
| `--pc` | The **product colour** in scope. Falls back to `--accent`. |
| `--glow`, `--glow-mix` | The accent glow behind heroes. |
| `--headline` | The gradient headings are painted with. |
| `--tile-mix`, `--tile-fg-mix` | How strongly monogram tiles take their product colour. |
| `--card-shadow` | Card elevation. |
| `--menu-bg` | The products dropdown, which must be opaque. |
| `--code-comment`, `--code-keyword`, `--code-string`, `--code-constant`, `--code-function`, `--code-type` | The syntax-highlighting palette. |

`--pc` is the one to know. It is set inline on every element scoped to a product, which is how one stylesheet tints five products differently — and why `product_theming = false` can collapse them all with a single rule.

## Syntax highlighting follows the theme

Code is highlighted at build time into **class names**, not inline colours: `hl-comment`, `hl-keyword`, `hl-string` and so on. Those classes resolve through the six `--code-*` tokens above, which are redefined in each theme state — which is why code goes light and dark with the rest of the page. Override those six tokens to change the code palette; see [Code blocks](~trail/writing/code-blocks).

## Fonts

Two variable fonts ship with the site and are served from `/assets/fonts/`: **Inter** for body text and **Space Grotesk** for headings. Both are self-hosted — no third-party font request — and their licences ship beside them, as the SIL Open Font License requires.

To use different fonts, add `@font-face` rules and a `font-family` override in your custom stylesheet. The built-in fonts are still emitted; there is no setting to suppress them.

## The custom stylesheet

```toml
custom_css = "custom.css"
```

The named file ships at `/assets/custom.css` and is loaded **after** the built-in stylesheet, on every page.

Overriding tokens goes furthest, and survives changes to trail's own CSS:

```css
:root {
  --accent: #0f766e;
  --card-shadow: none;
}
:root[data-theme="dark"] { --accent: #5eead4; }
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) { --accent: #5eead4; }
}
```

Beyond that it is ordinary CSS against trail's own class names — `.card`, `.sidebar`, `.admonition`, `.code-block` and the rest. Those class names are not a stable interface: they are what the compiled templates happen to emit today, and a trail upgrade may change them. Token overrides are the durable half.

## Injecting into the head

For anything that is not a stylesheet — a webfont link, an analytics snippet, extra meta tags — [`head_html`](~trail/building/configuring-the-site) names a file injected verbatim at the end of every page's `<head>`.

## What you cannot change

- **The templates.** They are compiled into the binary and are not user-replaceable. Page structure is trail's.
- **The layout**, beyond what CSS can reach.
- **The set of page types.** There is no way to add a new kind of page.

That is the trade trail makes: no plugin system and no theme directory, in exchange for a single binary with nothing to install and one appearance to maintain.
