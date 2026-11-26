---
packages:
  takumi-core: minor
---

# Support `revert`, `revert-layer`, and `revert-rule`

The three keywords roll a property back to a weaker declaration, as in Chrome: `revert` to the browser's default style, `revert-layer` to the cascade layer below, and `revert-rule` to the rule below. Without one, the property is `unset`. They work on custom properties and in `all` too.
