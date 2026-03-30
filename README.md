<div align="center">
  <img src="./assets/images/takumi.svg" alt="Takumi" width="64" />

# Takumi

**Render React components to images, animations, and video frames.**

One engine for Node.js, Edge, browsers, or embed the Rust crate directly.

[Documentation](https://takumi.kane.tw/docs/) · [Playground](https://takumi.kane.tw/playground)

</div>

## What you can build

### Static images

Open Graph cards, blog covers, social media graphics with proper typography.

### Animations

GIFs, WebP animations, or raw frames piped to FFmpeg for video.

### At scale

Batch processing with Node.js native / WebAssembly bindings.

## Install

```bash
bun i takumi-js
```

## Quick start

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

## Render animations

```tsx
import { render } from "takumi-js";

// Single frame at specific time
const frame = await render(<Scene />, {
  width: 1200,
  height: 630,
  timeMs: 500,
  keyframes: {
    move: {
      from: { transform: "translateX(0)" },
      to: { transform: "translateX(100px)" },
    },
  },
});

// Or pipe to FFmpeg for video
for (let i = 0; i < 120; i++) {
  const frame = await render(<Scene />, {
    timeMs: (i / 30) * 1000,
    format: "raw", // RGBA pixels
  });
  ffmpeg.stdin.write(frame);
}
```

## Use from Rust

```rust
use takumi::{
  layout::{node::Node, Viewport, style::{Length::Px, Style, StyleDeclaration}},
  resources::font::FontResource,
  rendering::{render, RenderOptions},
  GlobalContext,
};

let node = Node::container([Node::text("Hello").with_style(
  Style::default().with(StyleDeclaration::font_size(Px(32.0).into())),
)]);

let mut global = GlobalContext::default();
global.font_context_mut().load_and_store(
  FontResource::new(include_bytes!("font.woff2"))
);

let image = render(RenderOptions::builder()
  .viewport(Viewport::new((1200, 630)))
  .node(node)
  .global(&global)
  .build()
).unwrap();
```

## Capabilities

### Layout

Powered by [Taffy](https://github.com/DioxusLabs/taffy) for high-performance Flexbox, CSS Grid, and block layouts. Supports absolute positioning, z-index stacking, inline text spans, and nested inline blocks.

### Typography

Browser-grade text shaping and layout. Supports WOFF/WOFF2 fonts, emoji, and RTL languages out of the box.

### Runtime

Deploy anywhere. Ships with optimized native bindings for Node.js (via N-API), WebAssembly for Edge environments (Cloudflare Workers, browsers), or embed the native [Rust crate](https://docs.rs/takumi) directly.

### Keyframes Animation

Parses CSS `@keyframes` and the `animation` shorthand property. Supports percentage offsets, standard timing functions (`linear`, `ease`, `steps`, custom `cubic-bezier`), delays, fill modes, and iteration counts. Includes built-in Tailwind presets like `spin`, `ping`, `pulse`, and `bounce`.

### Styling

Full CSS stylesheet parsing and native Tailwind v4 support. Handles arbitrary values, linear and radial gradients, box shadows, blend modes, transforms, filters, and complex CSS selectors.

## Comparison

| Feature             | Satori | Takumi |
| ------------------- | ------ | ------ |
| Block/Inline layout | No     | Yes    |
| Flexbox             | Yes    | Yes    |
| Grid                | No     | Yes    |
| Animations          | No     | Yes    |
| WOFF Fonts          | Yes    | Yes    |
| WOFF2 fonts         | No     | Yes    |
| RTL languages       | No     | Yes    |

## Showcase

|                                 Takumi OG image [(source)](./example/twitter-images/components/og-image.tsx)                                 |                Package OG card [(source)](./example/twitter-images/components/package-og-image.tsx)                 |
| :------------------------------------------------------------------------------------------------------------------------------------------: | :-----------------------------------------------------------------------------------------------------------------: |
|                                       ![Takumi OG Image](./example/twitter-images/output/og-image.png)                                       |                      ![Package OG Image](./example/twitter-images/output/package-og-image.png)                      |
|                        **Prisma-style API card** [(source)](./example/twitter-images/components/prisma-og-image.tsx)                         |              **X-style social post** [(source)](./example/twitter-images/components/x-post-image.tsx)               |
|                                   ![Prisma OG Image](./example/twitter-images/output/prisma-og-image.png)                                    |                       ![X-style Post Image](./example/twitter-images/output/x-post-image.png)                       |
|                             **Keyframe Animation** [(source)](./example/ffmpeg-keyframe-animation/src/index.tsx)                             |                                **[shiki-image](https://github.com/pi0/shiki-image)**                                |
| [![Keyframe Animation](./example/ffmpeg-keyframe-animation/output/thumbnail.webp)](./example/ffmpeg-keyframe-animation/output/animation.mp4) | ![Shiki Image Example](https://raw.githubusercontent.com/pi0/shiki-image/refs/heads/main/test/.snapshot/image.webp) |

- [(Unofficial) Takumi Playground](https://takumi-playground.kapadiya.net/)

## Contributing

Read [CONTRIBUTING.md](./CONTRIBUTING.md).

## License

MIT or Apache-2.0

<br/>
<a href="https://vercel.com/oss">
  <img alt="Vercel OSS Program" src="https://vercel.com/oss/program-badge.svg" />
</a>
