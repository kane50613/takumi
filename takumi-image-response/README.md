# @takumi-rs/image-response

A universal `ImageResponse` implementation for Takumi in Next.js and other environments.

Checkout the migration guide [From Next.js ImageResponse](https://takumi.kane.tw/docs/migration/migrate-from-image-response) for more details.

## Installation

```bash
npm install @takumi-rs/image-response @takumi-rs/core @takumi-rs/helpers
```

## Usage

```tsx
import ImageResponse from "@takumi-rs/image-response";

export function GET(request: Request) {
  return new ImageResponse(<OgImage />, {
    width: 1200,
    height: 630,
    format: "webp",
    headers: {
      "Cache-Control": "public, immutable, max-age=31536000",
    },
  });
}
```

### Fonts

Takumi comes with full axis [Geist](https://vercel.com/font) and Geist Mono by default.

We have global fonts cache to avoid loading the same fonts multiple times.

If your environment supports top-level await, you can load the fonts in global scope and reuse the fonts array.

```tsx
const fonts = [
  {
    name: "Inter",
    data: await fetch("/fonts/Inter-Regular.ttf").then((res) => res.arrayBuffer()),
    style: "normal",
    weight: 400,
  },
];

new ImageResponse(<OgImage />, { fonts });
```

If your environment doesn't support top-level await, or just want the fonts to get garbage collected after initialization, you can load the fonts like this.

```tsx
let isFontsLoaded = false;

export function GET(request: Request) {
  const fonts = [];

  if (!isFontsLoaded) {
    isFontsLoaded = true;
    fonts = [
      {
        name: "Inter",
        data: await fetch("/fonts/Inter-Regular.ttf").then((res) => res.arrayBuffer()),
        style: "normal",
        weight: 400,
      },
    ];
  }

  return new ImageResponse(<OgImage />, { fonts });
}
```

### Bring-Your-Own-Renderer (BYOR)

If you want to use your own renderer instance, you can pass it to the `ImageResponse` constructor.

```tsx
import { Renderer } from "@takumi-rs/core";

const renderer = new Renderer();

new ImageResponse(<OgImage />, { renderer });
```

---

## WASM Usage

If you want to use this package in browser environment/cloudflare, you can import from the wasm entry point.

Make sure you have the `@takumi-rs/wasm` package installed as well.

```tsx
import { describe, expect, test } from "bun:test";
import init from "@takumi-rs/wasm";
import ImageResponse from "@takumi-rs/image-response/wasm";

// Or with bundler specific WASM resolution process,
// like cloudflare workers can be configured with wrangler.toml.
await init({
  module_or_path: fetch(
    import.meta.resolve("@takumi-rs/wasm/takumi_wasm_bg.wasm"),
  ),
});

export default {
  fetch() {
    return new ImageResponse(<div>Hello</div>);
  }
}
```
