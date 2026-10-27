---
packages:
  takumi-js: minor
---

# Type a renderer from either backend without an assertion

`takumi-js` now exports `Renderer`, the type of the `renderer` option. A renderer from `takumi-js/node` and one from `takumi-js/wasm` both satisfy it, so code that picks a backend at runtime no longer needs a type assertion.
