---
packages:
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  "@takumi-rs/helpers": minor
  takumi-js: minor
  takumi-paint: minor
  takumi-pdf: minor
  takumi-core: minor
---

# Renderers parse HTML themselves

`takumi-paint` and `takumi-pdf` take HTML strings as `@takumi-rs/core` and `@takumi-rs/wasm` do, PDF header and footer bands included, and `takumi-pdf` picks font subsets for page counters from the classes the cascade sees. `takumi-js` hands HTML to the renderer instead of parsing it in JavaScript, and keeps parsing it for a custom `renderer` from an older binding.
