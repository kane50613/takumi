# @takumi-rs/core

Native Node.js bindings for [Takumi](https://github.com/kane50613/takumi), an image rendering engine written in Rust.

This package ships the high-performance N-API runtime used by Takumi on Node.js environments.

## Install

```bash
npm install @takumi-rs/core
```

## Usage

```ts
import { Renderer } from "@takumi-rs/core";

const renderer = new Renderer();
const png = await renderer.render(
  {
    type: "Element",
    tag: "div",
    children: [{ type: "Text", value: "Hello from Takumi" }],
  },
  {
    width: 1200,
    height: 630,
  },
);
```

For JSX and HTML conversion helpers, use [`@takumi-rs/helpers`](https://npmjs.com/package/@takumi-rs/helpers).

## Documentation

- Integration guide: <https://takumi.kane.tw/docs/integration>
- API reference: <https://takumi.kane.tw/docs/api-reference>
- Repository: <https://github.com/kane50613/takumi>

If you are looking for WebAssembly bindings, take a look at the [@takumi-rs/wasm](https://npmjs.com/package/@takumi-rs/wasm) package.

## License

MIT or Apache-2.0
