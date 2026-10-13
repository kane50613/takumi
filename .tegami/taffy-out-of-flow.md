---
packages:
  "takumi-core": patch
  "takumi": patch
---

# Keep the static position of absolute boxes whose containing block is further up

An absolutely positioned box without insets now starts where it would have sat in its parent's flow, padding included, even when its containing block is an ancestor further up. It used to start at that ancestor's content edge. Chrome places it the same way.
