---
packages:
  "takumi": patch
---

# Skip unknown at-rules in `StyleSheet::parse`

An at-rule takumi does not implement, such as `@font-face` or `@page`, is dropped instead of failing the whole sheet.
