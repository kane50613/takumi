---
packages:
  takumi-core: minor
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  takumi-pdf: minor
  takumi-paint: minor
---

# Ignore a style value Takumi cannot read instead of failing the render

A node's `style` object now drops a declaration whose value is invalid, such as a negative `height`, and renders the rest, as a browser does with a `style` attribute. The JS packages report each dropped declaration through `console.warn`, with the message the render used to reject with. In Rust, `InvalidDeclarations::collect` returns those messages for the style objects a closure deserializes.
