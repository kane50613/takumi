---
packages:
  takumi-core: patch
---

# Generate `::before` and `::after` whose `content` is empty

A pseudo-element with `content: ""` now gets a box, so a clearfix, a decorative bar, or an absolutely positioned overlay drawn by one shows up. It used to be dropped because it held no text.
