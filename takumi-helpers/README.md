# @takumi-rs/helpers

Types and helper functions for [Takumi](https://takumi.kane.tw/docs/platforms/pick-your-platform).

## Example

```ts
import { container, text, image, style } from "@takumi-rs/helpers";

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

// Fetch takumi server or with @takumi-rs/core
```
