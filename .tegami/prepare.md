---
packages:
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  "@takumi-rs/helpers": minor
  takumi-js: minor
---

# `Renderer.prepare` fetches what a render needs ahead of it

`renderer.prepare(source, options)` styles an HTML string or node tree, fetches the images it draws, emoji and stylesheet images included, and loads the font subsets its text uses. `render`, `renderSvg`, and `measure` take the prepared source with only output options such as `format`, so the layout cannot change between the two calls. `takumi-js` now prepares each render this way, reading the source once instead of twice.

With a custom `fetch`, `prepare`, `prepareImages`, and the `takumi-js` `images` option now hand it relative URLs such as `/logo.png`, which it can resolve. Without one, only `http:` and `https:` URLs are fetched, as before.
