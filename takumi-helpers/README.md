# @takumi-rs/helpers

**Turn JSX and HTML into Takumi node trees, and load the fonts, images, and emoji a render needs.**

[Documentation](https://takumi.kane.tw/docs/helpers) · [GitHub](https://github.com/kane50613/takumi)

`takumi-js`, `takumi-pdf`, and `takumi-paint` call these helpers for you. Install this package when you use the [`@takumi-rs/core`](https://www.npmjs.com/package/@takumi-rs/core) or [`@takumi-rs/wasm`](https://www.npmjs.com/package/@takumi-rs/wasm) bindings directly, or to load fonts for `takumi-pdf`. With `takumi-js`, import the same helpers from `takumi-js/helpers`.

## Install

```bash
npm install @takumi-rs/helpers
```

React and Preact are optional peer dependencies. Install one of them to convert JSX.

## Render JSX with a binding

`fromJsx()` returns the node tree and the CSS it collected. Pass both to a binding:

```tsx
import { Renderer } from "@takumi-rs/core";
import { fromJsx } from "@takumi-rs/helpers/jsx";

const { node, css } = await fromJsx(<div tw="flex text-4xl">Hello</div>);

const png = await new Renderer().render(node, { width: 1200, height: 630, css });
```

## What it exports

- **`@takumi-rs/helpers/jsx`:** `fromJsx()` converts JSX to a node tree and its CSS.
- **`@takumi-rs/helpers/html`:** `fromHtml()` converts an HTML string to a node tree and its CSS. It is synchronous.
- **`@takumi-rs/helpers`:** `container()`, `text()`, and `image()` build nodes by hand. `googleFonts()` and `fontFromUrl()` load fonts. `prepareImages()` fetches the remote images a tree uses.
- **`@takumi-rs/helpers/emoji`:** `extractEmojis()` replaces emoji in text nodes with image nodes from a provider such as Twemoji.

## License

MIT or Apache-2.0
