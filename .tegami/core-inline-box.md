---
packages:
  takumi-core:
    type: minor
  takumi-pdf:
    type: patch
---

### Resolve inline boxes once, for every backend

`resolve_inline_box` places an inline box's replaced content or nested subtree, shared by the SVG and PDF backends.
