---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Join square border sides at the corners as Chrome does

Square borders with mixed styles, colours or opacities now meet at the corners the way Chrome draws them. Before, a uniform dashed or dotted border showed a diagonal seam at each corner, and translucent sides blended twice where they overlapped.
