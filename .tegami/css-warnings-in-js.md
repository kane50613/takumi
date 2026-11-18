---
packages:
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
---

# Unsupported CSS warns through `console.warn`

`@takumi-rs/core`, `@takumi-rs/wasm`, and `takumi-js` on either of them now report a CSS property takumi does not support through `console.warn` each time they parse a style sheet or `style` attribute that uses it. A render that reuses a `css` list the renderer still holds parsed does not warn again. Before, the declaration dropped without a word, so a typo such as `widht` changed the render silently. Other warnings takumi logs, such as an animated image frame that fails to decode, print the same way.
