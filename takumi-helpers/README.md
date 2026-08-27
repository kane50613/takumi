# @takumi-rs/helpers

**Utility functions and types for working with Takumi node trees.**

Convert JSX and HTML into the node trees Takumi's Rust engine renders. Load fonts and process emoji.

[Documentation](https://takumi.kane.tw/docs/helpers#parsing-templates) · [GitHub](https://github.com/kane50613/takumi)

## Installation

```bash
npm install @takumi-rs/helpers
```

## Features

### JSX to Node Tree

Convert React-like elements into a serializable node tree + CSS.

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

Find and replace emoji characters in text nodes with image nodes (Twemoji or custom).

```ts
import { extractEmojis } from "@takumi-rs/helpers/emoji";

const newNode = extractEmojis(oldNode, "twemoji");
```

## License

MIT or Apache-2.0
