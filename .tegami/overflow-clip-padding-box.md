---
"takumi": patch
---

# Clip overflow at the padding box

`overflow: hidden`, `overflow: clip` and `contain: paint` now clip a box's children at its padding edge, as CSS specifies and as the SVG backend already did. Children were clipped at the content edge, so a box with padding hid content that spilled into it.
