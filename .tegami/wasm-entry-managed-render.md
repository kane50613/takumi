---
packages:
  npm:takumi-js: minor
---

### Add managed render functions to `takumi-js/wasm`

`takumi-js/wasm` now exports `render`, `renderSvg`, and `renderAnimation`
pinned to the WASM backend, using the binary `@takumi-rs/wasm/auto` resolves.
The backend cache keeps separate slots for auto and forced-WASM loads, and the
shared renderer is cached per backend, so mixing both entries stays correct.
