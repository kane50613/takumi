---
packages:
  takumi-core: minor
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  takumi-pdf: minor
  takumi-paint: minor
---

# A style value Takumi cannot read is ignored instead of failing the render

A node's `style` object now drops a declaration whose value is invalid, such as a negative `height`, and renders the rest, as a browser does with a `style` attribute. Where a render used to reject, `console.warn` now prints the same message after `takumi: ignoring`, and in Rust the message is a `log` warning under the `takumi::style` target.
