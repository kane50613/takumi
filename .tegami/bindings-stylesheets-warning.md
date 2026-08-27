---
packages:
  "@takumi-rs/core":
    type: patch
  "@takumi-rs/wasm":
    type: patch
  "takumi-pdf":
    type: patch
---

### Warn when the bindings take the deprecated `stylesheets` option

`@takumi-rs/core` and `@takumi-rs/wasm` accept `stylesheets` as an alias for `css` but said nothing about it. The first call that passes the old name now writes one `console.warn`, whether it renders or measures.
