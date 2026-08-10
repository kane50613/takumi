---
packages:
  takumi-core:
    type: patch
---

### Accept `text-decoration: none`

`none` is the initial value of `text-decoration-line`, but the parser rejected it and failed the whole render. Both `text-decoration` and `text-decoration-line` now parse `none` as no decoration.
