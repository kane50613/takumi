---
packages:
  takumi-core: patch
---

# Tailwind utilities that tie sort numbers by value

When two utilities set the same property, `p-2 p-10` now applies `p-10` as Tailwind does, because digit runs in a name compare as numbers.
