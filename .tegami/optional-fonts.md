---
packages:
  "@takumi-rs/helpers":
    type: minor
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  takumi-pdf:
    type: minor
  takumi-paint:
    type: minor
  takumi-js:
    type: minor
---

# Render without a font that fails to load when the font is optional

A font in a render's `fonts` list takes `optional: true`. When an optional font fails to download, `allowUrl` refuses it, or its bytes do not decode, the render goes on without it instead of rejecting.

```tsx
await render(<div style={{ fontFamily: "Brand, Inter" }}>Hello</div>, {
  width: 1200,
  height: 630,
  fonts: [
    fontFromUrl("https://example.com/Inter.woff2"),
    fontFromUrl("https://cdn.example.com/Brand.woff2", { optional: true }),
  ],
});
```
