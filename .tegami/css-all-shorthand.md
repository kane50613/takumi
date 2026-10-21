---
packages:
  takumi-core: minor
---

# Support the `all` shorthand

`all: initial`, `all: inherit`, and `all: unset` now set every property except `direction`, `unicode-bidi`, and custom properties, as in Chrome. `all: revert` is ignored, as `revert` is on every other property.
