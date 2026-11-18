---
packages:
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
  takumi-core: patch
---

# Unsupported CSS warns through `console.warn`

`@takumi-rs/core`, `@takumi-rs/wasm`, and `takumi-js` on either of them now report a CSS property takumi does not support through `console.warn`, once per property in any letter case for the life of the process. Before, the declaration dropped without a word, so a typo such as `widht` changed the render silently. Other warnings takumi logs, such as an animated image frame that fails to decode, print the same way, once per message.
