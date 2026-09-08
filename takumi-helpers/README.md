# @takumi-rs/helpers

**Convert JSX and HTML into Takumi node trees and prepare fonts, images, and emoji.**

Use these helpers when working with the native or WebAssembly bindings directly. With `takumi-js`, import them from `takumi-js/helpers` and its subpaths.

[Documentation](https://takumi.kane.tw/docs/helpers#parsing-templates) · [GitHub](https://github.com/kane50613/takumi)

## Installation

```bash
npm install @takumi-rs/helpers
```

## Features

### JSX to Node Tree

Convert React-like elements into a serializable node tree and its CSS.

```tsx
import { fromJsx } from "@takumi-rs/helpers/jsx";

const { node, css } = await fromJsx(<div style={{ display: "flex" }}>Hello</div>);
```

### HTML to Node Tree

Parse HTML strings into Takumi nodes.

```ts
import { fromHtml } from "@takumi-rs/helpers/html";

const { node, css } = await fromHtml("<div style='color: red'>Hello</div>");
```

### Emoji Processing

Replace emoji in text nodes with image nodes from the selected provider.

```ts
import { extractEmojis } from "@takumi-rs/helpers/emoji";

const newNode = extractEmojis(oldNode, "twemoji");
```

## License

MIT or Apache-2.0
