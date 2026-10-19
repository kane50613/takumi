---
packages:
  takumi-core: patch
---

# Shape every size of a face with one font

A face builds its shaping tables once while it stays registered, instead of once for each size, variation, and render that uses it.
