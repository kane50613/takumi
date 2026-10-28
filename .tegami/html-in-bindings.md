---
packages:
  takumi-js: minor
  takumi-paint: minor
  takumi-pdf: minor
---

# Parse HTML strings in Rust in every renderer

`takumi-paint` and `takumi-pdf` take HTML strings as `@takumi-rs/core` and `@takumi-rs/wasm` do, and `takumi-pdf` takes them for header and footer bands too. `takumi-pdf` picks font subsets for page counters from the classes the cascade sees. With `emoji: "from-font"`, `takumi-js` hands an HTML string to the renderer instead of parsing it in JavaScript.
