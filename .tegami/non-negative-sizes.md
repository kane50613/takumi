---
packages:
  takumi-core: minor
---

# Reject negative padding and sizes

A negative `padding`, `width`, `height`, `min-*`, or `max-*`, including their logical forms, is now invalid, as in browsers: a stylesheet drops it and keeps the earlier declaration, and a style object reports it as an invalid value instead of using it.
