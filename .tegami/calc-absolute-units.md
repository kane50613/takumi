---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Sum a `calc()`'s absolute lengths into pixels

`calc()` now adds `cm`, `mm`, `in`, `pt`, `pc` and `q` into one pixel term, as CSS simplifies them, so an expression mixing several absolute units no longer fails to parse.
