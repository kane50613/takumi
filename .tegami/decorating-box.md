---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Place text decorations against the box that sets them

An underline now sits against the baseline and font of the element that sets it, and a line-through takes its height from that element's font, as Chrome's do. Under `text-fit`, decorations take their thickness and offset from the scaled font and stay crisp.
