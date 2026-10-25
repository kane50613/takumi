---
packages:
  takumi-core: patch
---

# Composite SVG layers faster

An SVG image now composites its opacity, clip, and filter layers with integer source-over, and converts filter colors between sRGB and linearRGB in one pass. A 2000 by 2000 SVG with a filter renders 25 to 40 ms faster. Edges can shift by one level of a channel.
