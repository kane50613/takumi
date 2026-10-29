---
packages:
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  "@takumi-rs/helpers": minor
  takumi-js: minor
---

# `Renderer.prepare` fetches what a render needs ahead of it

`renderer.prepare(source, options)` styles an HTML string or node tree, fetches the images it draws, and loads the font subsets its text uses. `render`, `renderSvg`, and `measure` take the prepared source with only output options such as `format`. `renderer.prepareAnimation(options)` does the same for every scene of an animation, and `renderAnimation` takes its result. `prepareImages` and `withRenderInput` from `@takumi-rs/helpers/renderer` are deprecated in favor of `prepare`.

`takumi-js` renders through `prepare`, so it fetches the images the styled tree draws. An image that `css`, a `<style>` element, or a `::before` rule sets now renders, an image under `display: none` is no longer fetched, and a custom `fetch` now receives relative URLs such as `/logo.png`.
