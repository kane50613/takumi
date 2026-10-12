---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Mix absolute units in `calc()`

`calc()` now adds `cm`, `mm`, `in`, `pt`, `pc` and `q` together, so mixing them no longer fails to parse.
