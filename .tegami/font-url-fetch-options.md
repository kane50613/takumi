---
packages:
  "@takumi-rs/core":
    type: patch
  "@takumi-rs/wasm":
    type: patch
  takumi-pdf:
    type: patch
  takumi-paint:
    type: patch
  takumi-js:
    type: patch
---

# Download fonts listed as URLs with the render's fetch options

A URL string in a render's `fonts` list now downloads with the `fetch`, `allowUrl`, `timeout`, `maxBytes`, and `signal` the render's images and `@font-face` files use, so `allowUrl` refuses it too.

```tsx
import { render } from "takumi-js";

await render(<div>Hello</div>, {
  width: 1200,
  height: 630,
  fonts: ["https://fonts.example.com/Brand.woff2"],
  images: { allowUrl: (url) => url.startsWith("https://cdn.example.com/") },
});
// Before: downloads the font
// After: rejects with "URL blocked by allowUrl policy: https://fonts.example.com/Brand.woff2"
```

- With a custom `fetch`, the font skips the renderer's default store, as images do.
- A render that waits on another render's download of the same font tries its own download when that one fails.
