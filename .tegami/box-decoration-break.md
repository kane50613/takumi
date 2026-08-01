---
packages:
  "cargo:takumi-core": minor
---

### Add box-decoration-break

`box-decoration-break: slice | clone` parses. The paged PDF backend uses it to decide how a box painted across page fragments closes its edges: `slice` (the default) leaves the edge at a break open, matching browser print behavior, while `clone` paints every fragment's complete borders and background. Cloned padding is paint-only and does not reserve layout space.
