---
packages:
  "cargo:takumi-core":
    type: minor
---

### `Node::alt` keeps empty values

`alt()` now returns `Some("")` for an explicitly empty attribute, so callers can tell a decorative image apart from a missing `alt`.
