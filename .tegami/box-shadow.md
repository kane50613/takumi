---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Paint `box-shadow` as Chrome does

- The first shadow in the list sits on top in every output.
- A spread shadow follows Chrome's corner radius, so square corners stay square.
- Shadows of scaled or rotated boxes blur by the transformed radius.
- Image output keeps fractional offsets.
- SVG no longer paints an outer shadow under a translucent box.
- PDF shadows no longer leave a hairline, fill a box they moved off, or stack their alpha when blurred.
