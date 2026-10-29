---
packages:
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  takumi-pdf: minor
  takumi-paint: minor
---

# `prepare` downloads the fonts `@font-face` rules load

An `@font-face` rule in `css` or a `<style>` element now loads its font. `prepare`, and the `render` and `ImageResponse` of `takumi-js`, download the `src` files the text reaches with the `images` fetch options, and move a rule on to its next `src` when a download fails. The renderer keeps the files within `fontCacheMaxBytes`, next to the fonts from `fonts` lists.
