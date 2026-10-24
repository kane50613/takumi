---
packages:
  takumi-core: minor
---

# Parse `filter` as Chrome does

`filter` now accepts `url(#id)`, which an HTML element ignores. A filter function with no argument uses its default amount, a negative amount is invalid, `hue-rotate()` takes only an angle with a unit or `0`, and `blur()` takes only a length.
