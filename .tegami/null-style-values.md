---
packages:
  "cargo:takumi-core": patch
---

### Ignore null style values

Skip `null` and `undefined` style declarations instead of failing to deserialize the style.
