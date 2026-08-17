---
packages:
  "@takumi-rs/core":
    type: patch
---

### Store `calc()` as its non-zero terms

`Length` shrinks from 84 to 20 bytes, and a computed style from 5.9KB to 2.3KB. A `calc()` expression mixing more than four distinct units now fails to parse.
