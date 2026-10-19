<div align="center">
  <img src="https://takumi.kane.tw/logo.svg" alt="Takumi" width="64" />

# @takumi-rs/core

**Native Node.js bindings for Takumi's image renderer.**

[Documentation](https://takumi.kane.tw/docs/) · [Playground](https://takumi.kane.tw/playground)

</div>

`@takumi-rs/core` renders Takumi node trees to images in Node.js and Bun. Most apps should install [`takumi-js`](https://www.npmjs.com/package/takumi-js) instead. It uses this package on Node.js and Bun, and it also accepts JSX and HTML and fetches remote images. Use `@takumi-rs/core` directly when you build node trees yourself.

For Cloudflare Workers and browsers, use [`@takumi-rs/wasm`](https://www.npmjs.com/package/@takumi-rs/wasm). For PDF documents, use [`takumi-pdf`](https://www.npmjs.com/package/takumi-pdf).

## Install

```bash
npm install @takumi-rs/core
```

Prebuilt binaries cover macOS, Linux (glibc and musl), and Windows, on x64 and arm64.

## Render a node tree

```ts
import { writeFile } from "node:fs/promises";
import { Renderer } from "@takumi-rs/core";

const renderer = new Renderer();

const png = await renderer.render(
  {
    type: "container",
    children: [{ type: "text", text: "Hello from Takumi" }],
  },
  { width: 1200, height: 630 },
);

await writeFile("output.png", png);
```

`render()` returns a PNG `Buffer` unless you set `format`. To build the tree from JSX or HTML, use [`@takumi-rs/helpers`](https://www.npmjs.com/package/@takumi-rs/helpers).

## Fonts

A built-in Geist font covers Latin text. Pass other fonts with each render:

```ts
import { readFile } from "node:fs/promises";

const png = await renderer.render(node, {
  width: 1200,
  height: 630,
  fonts: [await readFile("./Inter.ttf")],
});
```

To render many images with the same fonts, register them once with `renderer.registerFont()`.

## Renderer methods

| Method                   | Returns                 | Output                                   |
| :----------------------- | :---------------------- | :--------------------------------------- |
| `render(node, opts?)`    | `Promise<Buffer>`       | PNG, JPEG, WebP, ICO, or raw RGBA pixels |
| `renderSvg(node, opts?)` | `Promise<string>`       | An SVG document                          |
| `renderAnimation(opts)`  | `Promise<Buffer>`       | Animated WebP, APNG, or GIF              |
| `measure(node, opts?)`   | `Promise<MeasuredNode>` | The layout, without drawing              |

See the [node and style reference](https://takumi.kane.tw/docs/reference) and the [integration guides](https://takumi.kane.tw/docs/integration).

## License

MIT or Apache-2.0

Bundled third-party code is listed in the [third-party notices](https://github.com/kane50613/takumi/blob/master/THIRD-PARTY-NOTICES.md).
