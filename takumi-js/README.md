<div align="center">
  <img src="https://takumi.kane.tw/logo.svg" alt="Takumi" width="64" />

# takumi-js

**All-in-one Takumi package for Node.js and WebAssembly runtimes.**  
OG cards, banners, and lightweight animations from one Rust engine — no headless browser required.

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

## Entry points

| Import                    | Description                                                             |
| :------------------------ | :---------------------------------------------------------------------- |
| `takumi-js`               | `render()` + all shared types                                           |
| `takumi-js/response`      | `ImageResponse` class — drop-in compatible with `next/og`               |
| `takumi-js/node`          | Re-exports `@takumi-rs/core` (native napi-rs bindings)                  |
| `takumi-js/wasm`          | Re-exports `@takumi-rs/wasm` + `init()` for manual WASM initialization  |
| `takumi-js/helpers`       | Node tree utilities (`fromJsx`, `fromHtml`, `fetchResources`, …)        |
| `takumi-js/helpers/jsx`   | `fromJsx` — convert a React element to a Takumi node tree + stylesheets |
| `takumi-js/helpers/emoji` | `extractEmojis` — replace emoji characters with image nodes             |

## Runtime detection

`takumi-js` automatically selects the best backend:

- **Node.js** → native `@takumi-rs/core` (napi-rs)
- **Next.js Edge / Cloudflare Workers / browsers** → `@takumi-rs/wasm`

You can override this by passing a `module` option to `render()` or using `takumi-js/wasm` directly.

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

### Animated WebP

```tsx
import { Renderer } from "takumi-js/node";
import { fromJsx } from "takumi-js/helpers/jsx";
import { writeFile } from "node:fs/promises";

const renderer = new Renderer();

const { node, stylesheets } = await fromJsx(
  <div tw="w-full h-full flex items-center justify-center">
    <div tw="w-32 h-32 bg-blue-500 animate-spin rounded-lg" />
  </div>,
);

const animation = await renderer.renderAnimation({
  width: 400,
  height: 400,
  fps: 30,
  format: "webp",
  stylesheets,
  scenes: [{ durationMs: 1000, node }],
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
