---
name: takumi-usage
description: Guidelines, syntax, templates, API usage, and styling best practices for rendering static/animated images and vector SVG using Takumi.
---

# Takumi Usage & Best Practices

All-in-one guide for rendering with Takumi. Built-in layout rules, advanced styles, and performance.

## JS / TS API Reference

### Static Render (`render` / `renderSvg`)

```typescript
import { render, renderSvg } from "takumi-js";
const png = await render(node, options);
const svg = await renderSvg(node, options);
```

### Animation Render (`renderAnimation`)

```typescript
import { renderAnimation } from "takumi-js";
const webp = await renderAnimation({
  width: 400, height: 400, fps: 30, format: "webp", // 'webp' | 'gif' | 'apng'
  scenes: [{ node: <div />, durationMs: 1000 }]
});
```

### Options (`RenderOptions`)

- `fonts`: Custom font array `[{ name, url, weight, style }]`. URLs, local paths, or Buffers.
- `emoji`: Emoji fallback provider: `"twemoji"` (default), `"blob-emoji"`, `"openmoji"`, `"noto-emoji"`, or `"from-font"`.
- `images`: Pre-fetched images `[{ url, buffer }]` or fetch client/cache configuration.
- `stylesheets`: Array of raw global CSS stylesheet strings.

---

## Hidden Features (Deep-Dive from Source)

### 1. Auto-scaling Text (`text-fit`)

Scales inline text to fit its container box without overflow.

- Syntax: `text-fit: [ none | grow | shrink ] [ consistent | per-line | per-line-all ]? [percentage]?`
- Example: `tw="text-fit-grow-consistent"` or `style={{ textFit: "grow consistent 120%" }}`.

### 2. CSS Motion Paths (`offset-path`)

Positions or moves items along vector shapes, paths, or rays.

- Properties: `offset-path`, `offset-distance` (e.g. `50%`), `offset-rotate` (e.g. `45deg`).
- Values: `ray(<angle> <size> contain? at <position>?)`, `path("<svg path>")`, basic shapes (`circle()`, `polygon()`, `inset()`), or `<coord-box>` (`content-box`, `border-box`).

### 3. OpenType Features (`font-variation-settings`)

Natively supports variable fonts and features.

- Properties:
  - `font-variation-settings`: e.g. `"'wght' 750, 'wdth' 90"`.
  - `font-feature-settings`: e.g. `"'ss01' 1, 'kern' 1"`.

### 4. Graphic Filters & Effects

- `mix-blend-mode` & `background-blend-mode`: e.g., `multiply`, `screen`, `overlay`.
- `filter`: `blur()`, `brightness()`, `contrast()`, `drop-shadow()`, `grayscale()`, `hue-rotate()`, `invert()`, `opacity()`, `saturate()`, `sepia()`.
- `backdrop-filter`: Applied to containers.
- `clip-path`: Shape clipping via `polygon()`, `circle()`, `ellipse()`, `path()`, `inset()`.

### 5. Layout Engine

- CSS Grid: Fully supported (`grid-cols-X`, `gap-X`, custom rows/cols).
- Layout modes: Flexbox, block, inline-block, float.
- Sizing: `aspect-ratio`, `z-index`, `calc()`.

---

## Rust Crate API (`takumi`)

```rust
use takumi::{render, RenderOptions, Node};
let node = Node::from_html(html_str, None).unwrap();
let png_bytes = render(&node, &RenderOptions { width: 800, height: 600, ..Default::default() }).unwrap();
```
