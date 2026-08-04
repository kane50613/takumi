<div align="center">
  <img src="./assets/images/sticker.svg" alt="Takumi Sticker" height="96" />

# Takumi

**A Rust rendering engine that turns JSX, HTML, and node trees into images. No headless browser required.**

Render OpenGraph cards, animated GIFs, video frames, vector SVG, and paged PDFs from Node.js, Cloudflare Workers, browsers, or any Rust application.
Drop-in compatible with `next/og`.

[![npm version](https://img.shields.io/npm/v/takumi-js?label=takumi-js)](https://www.npmjs.com/package/takumi-js)
[![crates.io](https://img.shields.io/crates/v/takumi?label=takumi)](https://crates.io/crates/takumi)
[![npm downloads](https://img.shields.io/npm/dm/%40takumi-rs%2Fcore?label=downloads)](https://www.npmjs.com/package/@takumi-rs/core)
[![license](https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-blue)](#license)

[Documentation](https://takumi.kane.tw/docs/) · [Playground](https://takumi.kane.tw/playground) · [Showcase](https://takumi.kane.tw/showcase)

</div>

## Why Takumi

Takumi is a rendering pipeline built in Rust for one job: **turning markup and CSS into pixels**. It parses CSS, lays out the tree, shapes text, composites layers, and encodes the output inside a single binary. A headless-Chromium setup spends around **300 MB of RAM** and a browser cold start on the same OG card; Takumi spends **a function call**.

**One engine, every deployment target:**

- **Node.js** loads the native binding
- **Cloudflare Workers and browsers** load the WASM build
- **Rust applications** embed the `takumi` crate

Prebuilt binaries ship for macOS, Linux (glibc and musl), and Windows, on x64 and ARM64.

**CSS support reaches past the usual OG-image subset:**

- CSS Grid, block, inline, float
- `::before`, `::after`, `:is()`, `:where()`
- masks, `clip-path`, `backdrop-filter`, blend modes
- `background-clip: text`, conic gradients
- RTL text
- Tailwind v4 utilities, including arbitrary values

## Quick Start

```bash
bun i takumi-js
```

### Static image

```tsx
import { render } from "takumi-js";
import { writeFile } from "node:fs/promises";

const image = await render(
  <div tw="w-full h-full flex items-center justify-center bg-gradient-to-b from-blue-100 to-red-50">
    <h1 tw="text-6xl font-bold">Hello from Takumi</h1>
  </div>,
  { width: 1200, height: 630 },
);

await writeFile("./output.png", image);
```

### Fonts

Only a last-resort Latin font ships built in. Load the rest through `fonts`: a URL, raw bytes, or `googleFonts`. A weight range or an `axes` entry loads the variable font, so `font-variation-settings` drives its axes:

```tsx
import { render } from "takumi-js";
import { googleFonts } from "takumi-js/helpers";

const image = await render(
  <div
    tw="w-full h-full flex items-center justify-center"
    style={{
      fontSize: 72,
      fontFamily: "Fraunces",
      fontVariationSettings: "'opsz' 72, 'wght' 700",
    }}
  >
    Hello from Takumi
  </div>,
  {
    width: 1200,
    height: 630,
    fonts: googleFonts([{ name: "Fraunces", weight: "100..900", axes: { opsz: "9..144" } }]),
  },
);
```

Rendering many images? Register the fonts once on a `Renderer` and reuse it. See [Typography & Fonts](https://takumi.kane.tw/docs/typography-and-fonts).

### API route (`next/og`-compatible)

```tsx
import { ImageResponse } from "takumi-js/response";

export function GET() {
  return new ImageResponse(
    <div tw="w-full h-full flex items-center justify-center bg-gradient-to-b from-blue-100 to-red-50">
      <h1 tw="text-6xl font-bold">Hello from Takumi</h1>
    </div>,
    { width: 1200, height: 630 },
  );
}
```

### Animated WebP

```tsx
import { renderAnimation } from "takumi-js";
import { writeFile } from "node:fs/promises";

const animation = await renderAnimation({
  width: 400,
  height: 400,
  fps: 30,
  format: "webp",
  scenes: [
    {
      durationMs: 1000,
      node: (
        <div tw="w-full h-full flex items-center justify-center">
          <div tw="w-32 h-32 bg-blue-500 animate-spin rounded-lg" />
        </div>
      ),
    },
  ],
});

await writeFile("./output.webp", animation);
```

### Vector SVG

```tsx
import { renderSvg } from "takumi-js";
import { writeFile } from "node:fs/promises";

const svg = await renderSvg(
  <div tw="w-full h-full flex items-center justify-center bg-gradient-to-b from-blue-100 to-red-50">
    <h1 tw="text-6xl font-bold">Hello from Takumi</h1>
  </div>,
  { width: 1200, height: 630 },
);

await writeFile("./output.svg", svg);
```

### Paged PDF

The same engine writes multi-page vector PDFs through the [takumi-pdf](https://www.npmjs.com/package/takumi-pdf) package: selectable text, embedded subset fonts, page breaks, and repeating headers and footers.

```tsx
import { render } from "takumi-pdf";
import { writeFile } from "node:fs/promises";

const pdf = await render(<Invoice data={data} />, {
  size: "a4",
  footer: (
    <div tw="flex w-full justify-center text-[10px] text-gray-500">
      Page <span className="pageNumber" /> of <span className="totalPages" />
    </div>
  ),
});

await writeFile("invoice.pdf", pdf);
```

### Rust

```bash
cargo add takumi
```

Start from the [Rust example](./example/rust).

## Comparison

| Feature                            | `next/og` (Satori) |                            Takumi                             |
| :--------------------------------- | :----------------: | :-----------------------------------------------------------: |
| **Runtime**                        |    Node / Edge     |        Node, Edge, CF Workers, Browser, **Rust crate**        |
| **Template input**                 |    JSX / React     |     JSX, HTML strings, JSON node trees from any language      |
| **Layout**                         |      Flexbox       |          Flexbox, **CSS Grid**, block, inline, float          |
| **Selectors**                      |      Limited       | Complex selectors, `:is()`, `:where()`, `::before`, `::after` |
| **`backdrop-filter`, blend modes** |         ✗          |                              ✅                               |
| **Animated output**                |         ✗          |             **WebP / APNG / GIF / video frames**              |
| **Vector SVG output**              |     ✅ Native      |            ✅ **Plus raster and animated output**             |
| **Headless browser**               |         ✗          |                               ✗                               |
| **`ImageResponse` API**            |     ✅ Native      |                       ✅ **Compatible**                       |

Compare rendering output across providers at [image-bench.kane.tw](https://image-bench.kane.tw).

## Who's Using Takumi

- [Dcard](https://dcard.tw) renders post share images
- [TanStack](https://tanstack.com) renders OG images for its docs
- [Fumadocs](https://fumadocs.dev) generates its docs OG images
- [Nuxt OG Image](https://nuxtseo.com/docs/og-image/renderers/takumi) ships Takumi as a built-in renderer
- [Luma](https://lu.ma) renders event share images
- [shiki-image](https://github.com/pi0/shiki-image) turns syntax-highlighted code into images

More projects in the [showcase](https://takumi.kane.tw/showcase). Takumi is part of the [Vercel OSS Program](https://vercel.com/oss).

## Core Architecture

Takumi converts any template into a **node tree** with three node kinds: `container`, `image`, and `text`. That tree runs through:

1. **Layout** via [taffy](https://github.com/DioxusLabs/taffy): Flexbox, Grid, block, float, `calc()`, absolute positioning, z-index
2. **Text shaping** via [parley](https://github.com/linebender/parley) and [skrifa](https://github.com/googlefonts/fontations/tree/main/skrifa): WOFF/WOFF2 fonts, emoji, RTL, multi-span inline blocks
3. **Compositing**: stacking contexts, blend modes, filters, transforms, SVG via [resvg](https://github.com/linebender/resvg)
4. **Output**: PNG, JPEG, WebP, ICO for statics; GIF, APNG, WebP for animations; raw RGBA frames for video pipelines

The input contract is a node tree, so any template system that serializes to HTML or JSON can feed it: React, Svelte, Vue, plain strings, or your own serializer in any language.

A **time axis** threads through the pipeline: the renderer takes a timestamp, so a PNG is the tree at `t=0` and a GIF is the same tree sampled across `t`. CSS `@keyframes`, the `animation` shorthand, and Tailwind animation utilities (`animate-spin`, `animate-bounce`, arbitrary values) all resolve at render time.

The same layout drives a second backend: `renderSvg()` (Rust `render_svg`, behind the `svg-backend` feature) emits a real `<svg>` document built from `<rect>`, `<path>`, gradients, and glyph outlines, so you can ship scalable vector output instead of pixels. If you reached for [satori](https://github.com/vercel/satori) to get SVG, this is the drop-in path.

```mermaid
flowchart LR
    A[Templates] --> N[Node Tree] --> P[Rendering Pipeline]
    C[Stylesheets] --> P
    R[Resources] --> P
    D(Time Axis) -.-> P

    P --> F[(Raw Pixels)]
    P --> S[Vector SVG]

    F --> G[PNG / JPEG / WebP / ICO]
    F --> H[GIF / APNG]
    F --> I[Video frames]
```

## Showcase

|                                 Takumi OG image [(source)](./example/twitter-images/components/og-image.tsx)                                 |                Package OG card [(source)](./example/twitter-images/components/package-og-image.tsx)                 |
| :------------------------------------------------------------------------------------------------------------------------------------------: | :-----------------------------------------------------------------------------------------------------------------: |
|                                       ![Takumi OG Image](./example/twitter-images/output/og-image.png)                                       |                      ![Package OG Image](./example/twitter-images/output/package-og-image.png)                      |
|                        **Prisma-style API card** [(source)](./example/twitter-images/components/prisma-og-image.tsx)                         |              **X-style social post** [(source)](./example/twitter-images/components/x-post-image.tsx)               |
|                                   ![Prisma OG Image](./example/twitter-images/output/prisma-og-image.png)                                    |                       ![X-style Post Image](./example/twitter-images/output/x-post-image.png)                       |
|                             **Keyframe Animation** [(source)](./example/ffmpeg-keyframe-animation/src/index.tsx)                             |                                **[shiki-image](https://github.com/pi0/shiki-image)**                                |
| [![Keyframe Animation](./example/ffmpeg-keyframe-animation/output/thumbnail.webp)](./example/ffmpeg-keyframe-animation/output/animation.mp4) | ![Shiki Image Example](https://raw.githubusercontent.com/pi0/shiki-image/refs/heads/main/test/.snapshot/image.webp) |

**More examples:** [Next.js](./example/nextjs), [Cloudflare Workers](./example/cloudflare-workers), [TanStack Start](./example/tanstack-start), [Svelte](./example/svelte), [Rust](./example/rust), [ffmpeg keyframe animation](./example/ffmpeg-keyframe-animation)

- [(Unofficial) Takumi Playground](https://takumi-playground.kapadiya.net/)

## Contributing

Read [CONTRIBUTING.md](./CONTRIBUTING.md). Covers local setup, test commands, fixture workflow, and changelog process.

We welcome bug reports, feature requests, doc improvements, and new example integrations.

## License

MIT or Apache-2.0

<br/>
<a href="https://vercel.com/oss">
  <img alt="Vercel OSS Program" src="https://vercel.com/oss/program-badge.svg" />
</a>
