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

# A font one render lists no longer changes another render

Each render now draws with the fonts its own `fonts` list names and the fonts `registerFont` registered. Before, a renderer kept using the fonts earlier renders had listed, so the same request could come out differently.

```ts
await renderer.render(html, { fonts: [brandRegular] });
await renderer.render(html, { fonts: [brandBold] });
await renderer.render(html, { fonts: [brandRegular] });
// Before: the third render draws Brand with the bold face the second one listed
// After: the third render matches the first
```

A family that only another render listed now falls back to the next family. List the font in this render, or register it with `registerFont`.
