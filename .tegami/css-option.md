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

`css` takes inline CSS as one string or a list. `stylesheets` still works everywhere as a deprecated alias. On `takumi-js` and `takumi-pdf` it warns once and passing both throws; on the bindings `css` wins.
