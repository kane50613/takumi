---
packages:
  "@takumi-rs/core":
    type: minor
  "@takumi-rs/wasm":
    type: minor
  "takumi-js":
    type: minor
  "takumi-pdf":
    type: minor
---

### Rename the `stylesheets` render option to `css`

`css` takes inline CSS as one string or a list. The old `stylesheets` name still works everywhere and warns once on `takumi-js` and `takumi-pdf`.
