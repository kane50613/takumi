---
packages:
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  "@takumi-rs/helpers": minor
  takumi-js: minor
---

# Animation scenes take HTML, and `prepareImages` is deprecated

A `renderAnimation` scene can be an HTML string, its `<style>` rules applying to the animation, and font subsets follow the characters each scene shapes after the cascade. `takumi-js` hands HTML scenes to the renderer instead of parsing them in JavaScript. `prepareImages` is deprecated in favor of a renderer's `prepare`.
