<div align="center">
  <img src="https://takumi.kane.tw/logo.svg" alt="Takumi" width="64" />

# @takumi-rs/wasm

**WebAssembly bindings for [Takumi](https://github.com/kane50613/takumi), a Rust image rendering engine.**

Render OG cards, banners, and animations on Cloudflare Workers, edge runtimes, and browsers, no headless browser required.

[Documentation](https://takumi.kane.tw/docs/) · [Playground](https://takumi.kane.tw/playground)

</div>

## Install

```bash
npm install @takumi-rs/wasm
```

## Quick start

```ts
import init, { Renderer } from "@takumi-rs/wasm";

await init();

const renderer = new Renderer();

const png = await renderer.render(
  {
    type: "container",
    children: [{ type: "text", text: "Hello from Takumi" }],
  },
  { width: 1200, height: 630 },
);
```

Call `init()` once to load the WASM binary, then reuse the `Renderer`. Build node trees by hand, or generate them from JSX and HTML with [`@takumi-rs/helpers`](https://npmjs.com/package/@takumi-rs/helpers).

## Documentation

- Integration guide: <https://takumi.kane.tw/docs/integration>
- API reference: <https://takumi.kane.tw/docs/api-reference>
- Repository: <https://github.com/kane50613/takumi>

For Node.js, use the native [@takumi-rs/core](https://npmjs.com/package/@takumi-rs/core) bindings.

## License

MIT or Apache-2.0
