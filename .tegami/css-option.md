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

`css` takes inline CSS as one string or a list. `stylesheets` stays as a deprecated alias everywhere; on `takumi-js` and `takumi-pdf` passing both throws, and on the bindings `css` wins.
