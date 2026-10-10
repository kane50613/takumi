---
packages:
  "takumi": patch
---

# Apply clips faster

Nested clips, such as `overflow: hidden` or `border-radius` inside another, and layers from `opacity`, `filter` or a blend mode inside a clip are cut a run of pixels at a time. Pages with nested clips or large blurred backgrounds render up to 50% faster.
