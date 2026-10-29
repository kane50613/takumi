---
packages:
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  takumi-js: minor
  takumi-paint: minor
  takumi-pdf: minor
---

# Parse HTML strings in Rust in every renderer

`takumi-paint` and `takumi-pdf` take HTML strings as `@takumi-rs/core` and `@takumi-rs/wasm` do, and `takumi-pdf` takes them for header and footer bands too. A `renderAnimation` scene can be an HTML string, its `<style>` rules applying to the animation. With `emoji: "from-font"`, `takumi-js` hands HTML to the renderer instead of parsing it in JavaScript.
