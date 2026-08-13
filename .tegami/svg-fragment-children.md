---
packages:
  "@takumi-rs/helpers":
    type: patch
---

### Render fragment, memo and forwardRef children inside `<svg>`

Inline SVG silently dropped any child it could not map to a tag name. Fragments, `memo` and `forwardRef` components now serialize the same way React does.
