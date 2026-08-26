---
packages:
  "@takumi-rs/core":
    type: major
  "@takumi-rs/wasm":
    type: major
  "takumi-js":
    type: minor
  "takumi-pdf":
    type: minor
---

### Rename the `stylesheets` render option to `css`

`css` takes inline CSS as one string or a list. On `takumi-js` and `takumi-pdf`, `stylesheets` stays as a deprecated alias and passing both throws. The `@takumi-rs/core` and `@takumi-rs/wasm` bindings rename the field outright.
