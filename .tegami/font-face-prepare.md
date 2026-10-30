---
packages:
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  takumi-pdf: minor
  takumi-paint: minor
---

# `prepare` downloads the fonts `@font-face` rules load

An `@font-face` rule in `css` or a `<style>` element now loads its font. `prepare`, and the `render` and `ImageResponse` of `takumi-js`, download the `src` files the text reaches with the `images` fetch options. When a download fails, that document's rule moves on to its next `src`, and the next `prepare` tries the file again. The renderer keeps the files next to the fonts from `fonts` lists, and with `fontCacheMaxBytes` set, releases the least recently used past that budget.
