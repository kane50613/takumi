---
packages:
  "@takumi-rs/core": patch
  "@takumi-rs/wasm": patch
  takumi-core: patch
---

# Unsupported CSS warns through `console.warn`

`@takumi-rs/core`, `@takumi-rs/wasm`, and `takumi-js` on either of them now report a CSS property takumi does not support through `console.warn`, once per property for the life of the process. Before, the declaration dropped without a word, so a typo such as `widht` changed the render silently.

```text
takumi: ignored the property `widht` at line 1, column 8 of its style sheet or `style` attribute, since takumi does not support it. Check its spelling.
```

The other warnings takumi logs, such as an animated image frame that fails to decode, reach `console.warn` too, once per message. In Rust, the `log` record of that warning carries the property name under the `property` key.
