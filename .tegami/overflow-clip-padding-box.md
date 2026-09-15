---
packages:
  "takumi": patch
---

# Clip overflow at the padding box

`overflow: hidden`, `overflow: clip` and `contain: paint` now clip at the padding edge, as CSS specifies. Content that spilled into a padded box's padding used to be cut at the content edge.
