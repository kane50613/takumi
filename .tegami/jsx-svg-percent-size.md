---
packages:
  takumi-core: patch
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
  "@takumi-rs/helpers": patch
---

# A JSX `<svg>` or `<img>` sized with `"100%"` or `"24px"` draws at that size

A JSX `<svg>` or `<img>` whose `width` or `height` was a string with a unit, such as `"100%"`, `"2em"`, or `"24px"`, drew nothing. It now draws at that size, as the same markup passed as HTML does.

```tsx
<div style={{ display: "flex", width: 200, height: 100 }}>
  <svg width="100%" height="50%" viewBox="0 0 2 1">
    …
  </svg>
</div>
// Before: the svg draws nothing
// After: the svg's box is 200 × 50, since a percentage resolves against the containing block
```

- An `<svg>` reads `width` and `height` as CSS lengths, such as `2em`, and ignores a negative one.
- An `<img>` reads them as HTML does, as pixels or a percentage, so `width="2em"` is 2 pixels.
