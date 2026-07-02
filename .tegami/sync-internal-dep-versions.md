---
packages:
  npm:takumi-js: patch
---

### Pin `@takumi-rs/*` dependencies to the matching release

`takumi-js` resolved its `@takumi-rs/core`, `@takumi-rs/helpers`, and `@takumi-rs/wasm`
dependencies to an older release than itself, so `takumi-js/response` imported a helper
the pinned `@takumi-rs/helpers` did not yet export and failed to load. The internal
dependencies now track the same release as `takumi-js`.
