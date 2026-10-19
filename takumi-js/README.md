<div align="center">
  <img src="https://takumi.kane.tw/logo.svg" alt="Takumi" width="64" />

# takumi-js

**Render JSX, HTML, and CSS to images and animations. No headless browser.**

[Documentation](https://takumi.kane.tw/docs/) · [Playground](https://takumi.kane.tw/playground)

</div>

`takumi-js` is Takumi's package for images. It renders PNG, JPEG, WebP, SVG, and animated WebP, APNG, or GIF. Use it for social cards, banners, charts, and other generated graphics.

Takumi's packages share one layout engine, with text and inline layout ported from Chromium. For PDF documents, use [`takumi-pdf`](https://www.npmjs.com/package/takumi-pdf). To draw the laid-out boxes and text with your own renderer, use [`takumi-paint`](https://www.npmjs.com/package/takumi-paint).

## Install

```bash
npm install takumi-js
```

## Render an image

Save this as `image.tsx` and run `bun image.tsx`. In Node.js, compile the JSX first.

```tsx
import { render } from "takumi-js";
import { writeFile } from "node:fs/promises";

const png = await render(
  <div tw="w-full h-full flex items-center justify-center bg-linear-to-b from-blue-100 to-red-50">
    <h1 tw="text-6xl font-bold">Hello from Takumi</h1>
  </div>,
  { width: 1200, height: 630 },
);

await writeFile("output.png", png);
```

`render()` also accepts an HTML string or a Takumi node tree. It returns PNG bytes unless you set `format`.

Takumi never reads system fonts. A built-in Geist font covers Latin text. Load fonts for other scripts through the `fonts` option.

## Serve an image

`ImageResponse` extends the web `Response`, so any runtime with `Request` and `Response` can return it. Its API matches `next/og`.

```tsx
import { ImageResponse } from "takumi-js/response";

export function GET() {
  return new ImageResponse(<div tw="w-full h-full flex items-center justify-center">Hello</div>, {
    width: 1200,
    height: 630,
  });
}
```

## Other outputs

| Function                                                              | Returns                                    |
| :-------------------------------------------------------------------- | :----------------------------------------- |
| [`render()`](https://takumi.kane.tw/docs/output-formats)              | PNG, JPEG, WebP, ICO, or raw RGBA bytes    |
| [`renderSvg()`](https://takumi.kane.tw/docs/output-formats)           | An SVG document string                     |
| [`renderAnimation()`](https://takumi.kane.tw/docs/keyframe-animation) | Animated WebP, APNG, or GIF bytes          |
| [`measure()`](https://takumi.kane.tw/docs/measure-api)                | The laid-out node tree, without drawing it |

## Runtimes

`takumi-js` picks a binding from your runtime's import conditions:

- **Node.js and Bun:** the native [`@takumi-rs/core`](https://www.npmjs.com/package/@takumi-rs/core) binding.
- **Cloudflare Workers, Vercel Edge, Deno, and browsers:** the WebAssembly [`@takumi-rs/wasm`](https://www.npmjs.com/package/@takumi-rs/wasm) binding.

To use WebAssembly on Node.js, pass the binary as the `module` option. See [Troubleshooting](https://takumi.kane.tw/docs/troubleshooting) when a bundler picks the wrong entry.

## Migrate from another renderer

- **`satori`:** replace `satori()` with `renderSvg()`, or call `render()` for image bytes. See the [Satori comparison](https://takumi.kane.tw/docs/comparison-to-satori).
- **`next/og`:** change the `ImageResponse` import. Keep explicit Flexbox styles, because a bare `<div>` defaults to block layout. See the [`next/og` migration guide](https://takumi.kane.tw/docs/comparison-to-satori#migrate-from-nextog).
- **Puppeteer screenshots:** pass JSX or HTML to `render()`. Scripts do not run, and Takumi supports less CSS than Chrome.

## License

MIT or Apache-2.0
