---
packages:
  takumi-core: patch
---

# Apply `opacity` to SVG text spans

The `opacity` of a `tspan`, `textPath`, or `a` in SVG text now fades the characters it holds, as in Chrome. It was ignored before.
