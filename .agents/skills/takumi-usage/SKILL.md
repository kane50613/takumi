---
name: takumi-usage
description: Guidelines, syntax, templates, API usage, and styling best practices for rendering static/animated images and vector SVG using Takumi.
---

# Takumi Usage & Best Practices

All-in-one guide for rendering with Takumi. Built-in layout rules, advanced styles, and performance.

## JS / TS API Reference

### 1. Static Render (`render` & `renderSvg`)

Renders JSX templates, HTML strings, or JSON node trees to raster buffers (PNG/JPEG/WebP) or vector SVG markup.

```typescript
import { render, renderSvg } from "takumi-js";

// Renders dynamic JSX layout directly to PNG bytes
const pngBuffer = await render(
  <div tw="w-full h-full bg-zinc-950 flex items-center justify-center">
    <h1 tw="text-white text-6xl">Takumi Engine</h1>
  </div>,
  {
    width: 1200,
    height: 630,
    format: "png", // "png" | "jpeg" | "webp"
    quality: 90    // (Optional) For JPEG/WebP formats
  }
);

// Renders to responsive XML SVG string containing glyph path vector data
const svgString = await renderSvg(
  <div tw="w-full h-full bg-zinc-900">
    <span tw="text-white">Scalable Vector</span>
  </div>,
  { width: 800, height: 400 }
);
```

### 2. Animated Render (`renderAnimation`)

Compiles sequences of scenes into animated WebP, GIF, or APNG buffers.

```typescript
import { renderAnimation } from "takumi-js";

const animatedBuffer = await renderAnimation({
  width: 400,
  height: 400,
  fps: 30,
  format: "webp", // "webp" | "gif" | "apng"
  quality: 80,    // Compression quality (0-100)
  scenes: [
    {
      durationMs: 1500,
      node: (
        <div tw="w-full h-full bg-black flex items-center justify-center">
          <div tw="w-24 h-24 bg-rose-500 animate-bounce rounded-full" />
        </div>
      )
    }
  ]
});
```

### 3. Edge / Framework Integration (`ImageResponse`)

A next/og compatible handler designed for Edge runtimes, Cloudflare Workers, Next.js API endpoints, and standard HTTP server frameworks.

```typescript
import { ImageResponse } from "takumi-js/response";

export function GET() {
  return new ImageResponse(
    <div tw="w-full h-full bg-slate-900 flex items-center justify-center">
      <h1 tw="text-white text-5xl">Dynamic OG Card</h1>
    </div>,
    {
      width: 1200,
      height: 630
    }
  );
}
```

### 4. Parsing Helpers (`takumi-js/helpers/*`)

Low-level utilities for compiling layout elements without executing the full renderer.

```typescript
import { fromHtml } from "takumi-js/helpers/html";
import { fromJsx } from "takumi-js/helpers/jsx";

// Converts raw HTML template string into JSON layout node tree
const htmlNodeTree = fromHtml(
  "<div class='flex'><span>Hello</span></div>",
  { max_depth: 512 }
);

// Compiles React ReactNode elements into JSON layout node tree
const jsxNodeTree = fromJsx(
  <div tw="flex"><span>Hello</span></div>
);
```

---

## Options Configuration (`RenderOptions`)

- `fonts`: An array of custom fonts `[{ name, url, weight, style }]`. Values can point to remote URLs (WOFF2/WOFF/TTF), local file paths, or binary Node.js `Buffer` arrays.
- `emoji`: Configures the emoji fallback strategy: `"twemoji"` (default), `"blob-emoji"`, `"openmoji"`, `"noto-emoji"`, or `"from-font"` (uses custom loaded font glyphs).
- `images`: Option to pass pre-fetched image arrays `[{ url, buffer }]` or supply a shared caching fetch-client configuration.
- `stylesheets`: An array of raw global CSS string sheets to inject into the rendering layout context.

---

## Hidden Layout Capabilities (Deep-Dive)

### 1. Auto-scaling Text (`text-fit`)

Automatically scales font-size to fit the containing inline line-box width, avoiding clipping or wrapping.

- **Syntax**: `text-fit: [ none | grow | shrink ] [ consistent | per-line | per-line-all ]? [percentage]?`
- **Example**: `tw="text-fit-grow-consistent"` or `style={{ textFit: "grow consistent 120%" }}`.

### 2. CSS Motion Paths (`offset-path`)

Enables complex placement and animation sequences of elements along custom vector paths or geometric rays.

- **Properties**: `offset-path`, `offset-distance` (percentage), `offset-rotate` (angle / auto).
- **Syntax**: `ray(<angle> <size> contain? at <position>?)` or `path("<svg path command>")` or basic shapes (`circle()`, `polygon()`, `inset()`).

### 3. OpenType Typography Settings

Provides fine-grained control over font rendering features and variable layout configurations.

- `font-variation-settings`: Configures variable font axis settings (e.g. `"'wght' 750, 'wdth' 90"`).
- `font-feature-settings`: Enables OpenType font ligatures, kerning, and variants (e.g. `"'ss01' 1, 'kern' 1"`).

### 4. Custom Filters & Graphics

- `filter`: Supports `blur()`, `brightness()`, `contrast()`, `drop-shadow()`, `grayscale()`, `hue-rotate()`, `invert()`, `opacity()`, `saturate()`, `sepia()`.
- `mix-blend-mode` & `background-blend-mode`: Layer blending modes (e.g. `multiply`, `screen`, `overlay`).
- `backdrop-filter`: Applies filter effects to elements behind the container.
- `clip-path`: Clips elements via shapes: `polygon(...)`, `circle(...)`, `inset(...)`.

### 5. Layout Defaults

- CSS Grid support: Supports `grid-cols-X`, `gap-X`, flex, and block layout.
- Tag Presets: Custom HTML styles are pre-mapped for tags: `h1`–`h6`, `div`, `pre` (keeps space layout), `strong`, `em`, `blockquote`, and `hr`.
- Line Breaks: `<br>` tags parse to `\n` in text elements automatically.

---

## Rust Crate API (`takumi`)

<!-- keep in sync with the doctest in takumi/src/lib.rs -->

```rust
use takumi::prelude::*;
use takumi::render;

let node = Node::container([Node::text("Hello, world!").with_style(
  Style::default().with(StyleDeclaration::font_size(Length::Px(32.0).into())),
)]);

let mut fonts = Fonts::default();
fonts.register(FontResource::new(include_bytes!(
  "../../assets/fonts/geist/Geist[wght].woff2"
)))?;

let options = RenderOptions::builder()
  .viewport(Viewport::new((1200, 630)))
  .node(node)
  .fonts(&fonts)
  .build();

let image = render(options)?;
```

To build the node tree from HTML instead, enable the `from-html` feature and use `from_html`:

```rust
use takumi::prelude::*;
use takumi::{from_html, render};

let mut fonts = Fonts::default();
fonts.register(FontResource::new(include_bytes!(
  "../../assets/fonts/geist/Geist[wght].woff2"
)))?;

let html = r#"<div style="background: red; width: 100%; height: 100%;"></div>"#;
let node = from_html(html, FromHtmlOptions::default())?;

let options = RenderOptions::builder()
  .viewport(Viewport::new((1200, 630)))
  .node(node)
  .fonts(&fonts)
  .build();

let image = render(options)?;
```
