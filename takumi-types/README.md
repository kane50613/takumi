# takumi-types

Types and helper functions for [takumi](https://github.com/kane50613/takumi).

## Example

```ts
import { container, text, image, style } from "takumi-types";

const root = container({
  width: 1200,
  height: 630,
  children: [
    text(
      style({
        font_size: 24,
        font_weight: 700,
        color: 0xffffff,
      }),
      "Hello, world!"
    ),
  ],
});

const response = await fetch("https://your-takumi-server/image", {
  method: "POST",
  body: JSON.stringify({
    root,
  }),
});

const image = await response.arrayBuffer();
```
