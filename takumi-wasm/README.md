<div align="center">
  <img src="https://takumi.kane.tw/logo.svg" alt="Takumi" width="64" />

# @takumi-rs/wasm

**WebAssembly bindings for Takumi's image renderer.**

[Documentation](https://takumi.kane.tw/docs/) · [Playground](https://takumi.kane.tw/playground)

</div>

`@takumi-rs/wasm` renders Takumi node trees to images where native addons cannot load: Cloudflare Workers, edge runtimes, and browsers. Most apps should install [`takumi-js`](https://www.npmjs.com/package/takumi-js) instead. It loads this package on those runtimes, and it also accepts JSX and HTML and fetches remote images.

On Node.js and Bun, use the native [`@takumi-rs/core`](https://www.npmjs.com/package/@takumi-rs/core) binding. For PDF documents, use [`takumi-pdf`](https://www.npmjs.com/package/takumi-pdf).

## Install

```bash
npm install @takumi-rs/wasm
```

## Render a node tree

Load the WebAssembly binary once with `init()`, then reuse one `Renderer`. This example uses a bundler, such as Vite or webpack, that resolves the binary's URL:

```ts
import init, { Renderer } from "@takumi-rs/wasm";
import wasmUrl from "@takumi-rs/wasm/wasm-url";

await init({ module_or_path: wasmUrl });

const renderer = new Renderer();

const png = await renderer.render(
  {
    type: "container",
    children: [{ type: "text", text: "Hello from Takumi" }],
  },
  { width: 1200, height: 630 },
);
```

`render()` returns a PNG `Uint8Array` unless you set `format`. WebP output is always lossless in this binding. To build the tree from JSX or HTML, use [`@takumi-rs/helpers`](https://www.npmjs.com/package/@takumi-rs/helpers).

## Load the binary on other runtimes

- **Cloudflare Workers:** import the module from `@takumi-rs/wasm/takumi_wasm_bg.wasm` and pass it as `module_or_path`.
- **Node.js and Bun:** import from `@takumi-rs/wasm/node`. It loads the binary on import, so skip `init()`.

See [Troubleshooting](https://takumi.kane.tw/docs/troubleshooting) when a bundler cannot find the binary.

## License

MIT or Apache-2.0

Bundled third-party code is listed in the [third-party notices](https://github.com/kane50613/takumi/blob/master/THIRD-PARTY-NOTICES.md).
