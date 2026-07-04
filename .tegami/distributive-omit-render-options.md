---
packages:
  npm:takumi-js:
    replay:
      - exit-prerelease(npm:takumi-js)
---

### Keep the format tagged union on `render` options

`render`, `renderSvg`, and `renderAnimation` used a non-distributive `Omit` that
collapsed the `format`/`quality`/`lossless` union, so `{ format: "webp", quality: 80 }`
stopped type-checking. A distributive `Omit` restores it.
