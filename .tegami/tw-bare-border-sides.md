---
packages:
  "cargo:takumi-core": patch
---

### Render bare `border-t`/`border-r`/`border-b`/`border-l`/`border-x`/`border-y`

The side utilities only parsed with a width suffix, so plain `border-b` drew nothing.
