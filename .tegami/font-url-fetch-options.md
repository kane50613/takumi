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

A URL string in a render's `fonts` list now downloads with the `fetch`, `allowUrl`, `timeout`, `maxBytes`, and `signal` the render's images use, so `allowUrl` refuses it too.

```tsx
await render(<div>Hello</div>, {
  width: 1200,
  height: 630,
  fonts: ["https://fonts.example.com/Brand.woff2"],
  images: { allowUrl: (url) => url.startsWith("https://cdn.example.com/") },
});
// Before: downloads the font
// After: rejects with "URL blocked by allowUrl policy: https://fonts.example.com/Brand.woff2"
```
