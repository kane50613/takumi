<div align="center">
  <img src="https://takumi.kane.tw/logo.svg" alt="Takumi" width="64" />

# takumi-js

**Render JSX to SVG or images. Drop-in next/og replacement.**

OG cards, banners, and lightweight animations from one Rust engine, no headless browser required.

[Documentation](https://takumi.kane.tw/docs/) · [Playground](https://takumi.kane.tw/playground)

</div>

## Install

```bash
bun add takumi-js
# or
npm install takumi-js
```

## Quick start

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

## Runtime detection

`takumi-js` selects the backend for the runtime:

- **Node.js / Bun** → native `@takumi-rs/core` (napi-rs)
- **Next.js Edge / Cloudflare Workers / browsers** → `@takumi-rs/wasm`

Override it with a `module` option on `render()`, or import `takumi-js/wasm` directly.

## Examples

### `next/og`-compatible API route

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

### Render SVG

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

### Bun server

```tsx
import { ImageResponse } from "takumi-js/response";
import { serve } from "bun";

serve({
  fetch() {
    return new ImageResponse(
      <div tw="w-full h-full flex items-center justify-center bg-[linear-gradient(to_bottom,#dbf4ff,#fff1f1)]">
        <h1 tw="text-6xl font-bold">Hello from Takumi 👋</h1>
      </div>,
      { width: 1200, height: 630 },
    );
  },
  port: 3000,
});
```

## License

MIT or Apache-2.0
