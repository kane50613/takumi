---
packages:
  takumi-core: patch
---

# Apply `unicode-bidi` and `direction` to SVG text

SVG text now orders its characters with the `unicode-bidi` and `direction` of the `text` and of each `tspan`, `textPath`, and `a`, as in Chrome, so an override or an isolate takes effect and an RTL `text` starts its paragraph right to left.
