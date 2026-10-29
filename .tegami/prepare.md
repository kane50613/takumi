---
packages:
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  "@takumi-rs/helpers": minor
  takumi-js: minor
---

# `Renderer.prepare` fetches what a render needs ahead of it

`renderer.prepare(source, options)` styles an HTML string or node tree, fetches the images it draws, and loads the font subsets its text uses. `render`, `renderSvg`, and `measure` take the prepared source with only output options such as `format`. `renderer.prepareAnimation(options)` does the same for every scene of an animation, and `renderAnimation` takes its result. `prepareImages` and `withRenderInput` from `@takumi-rs/helpers/renderer` are deprecated in favor of `prepare`.

`takumi-js` now asks the renderer which images the styled tree draws, then fetches those. A `background-image`, `mask-image`, or `list-style-image` set by `css`, a `<style>` element, or a `::before` rule now renders instead of being left out, and an image under `display: none` is no longer fetched. With a custom `fetch`, relative URLs such as `/logo.png` now reach it. Without one, only `http:` and `https:` URLs are fetched, as before. `renderAnimation` styles each scene with the `<style>` elements of every HTML scene, as it renders them, before it reads what to fetch.
