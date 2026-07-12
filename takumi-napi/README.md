<div align="center">
  <img src="https://takumi.kane.tw/logo.svg" alt="Takumi" width="64" />

# @takumi-rs/core

**Native Node.js bindings for [Takumi](https://github.com/kane50613/takumi), a Rust image rendering engine.**

The high-performance N-API runtime that turns node trees into OG cards, banners, and animations, no headless browser required.

[Documentation](https://takumi.kane.tw/docs/) · [Playground](https://takumi.kane.tw/playground)

</div>

## Install

```bash
npm install @takumi-rs/core
```

## Quick start

```ts
import { Renderer } from "@takumi-rs/core";

const renderer = new Renderer();

const png = await renderer.render(
  {
    type: "container",
    children: [{ type: "text", text: "Hello from Takumi" }],
  },
  { width: 1200, height: 630 },
);
```

`render` takes a node tree and returns a PNG `Buffer`. Build the tree by hand, or generate it from JSX and HTML with [`@takumi-rs/helpers`](https://npmjs.com/package/@takumi-rs/helpers).

## Fonts

Pass fonts on the render options, no separate registration step:

```ts
import { readFile } from "node:fs/promises";

const png = await renderer.render(node, {
  width: 1200,
  height: 630,
  fonts: [await readFile("./Inter.ttf")],
});
```

## Renderer

| Method                   | Returns           | Description                       |
| :----------------------- | :---------------- | :-------------------------------- |
| `render(node, opts?)`    | `Promise<Buffer>` | Raster image (PNG, JPEG, WebP, …) |
| `renderSvg(node, opts?)` | `Promise<string>` | Vector SVG document               |
| `renderAnimation(opts)`  | `Promise<Buffer>` | Animated WebP, APNG, or GIF       |
| `measure(node, opts?)`   | `Promise<...>`    | Layout without rasterizing        |

## Documentation

- Integration guide: <https://takumi.kane.tw/docs/integration>
- API reference: <https://takumi.kane.tw/docs/api-reference>
- Repository: <https://github.com/kane50613/takumi>

For WebAssembly runtimes (Cloudflare Workers, browsers, edge), use [@takumi-rs/wasm](https://npmjs.com/package/@takumi-rs/wasm).

## License

MIT or Apache-2.0
