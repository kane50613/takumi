---
packages:
  takumi-core: patch
---

# Lay out nested inline blocks in linear time

Each level of nested `inline-block` boxes laid out the level below it several times over, so a few levels deep took minutes. An inline block now keeps the layout it took in each available space.
