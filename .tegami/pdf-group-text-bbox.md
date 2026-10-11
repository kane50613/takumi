---
packages:
  "takumi-pdf": patch
---

# Stop cutting off the end of text inside a transparency group

Text under `opacity`, a blend mode or a mask lost the right side of its last glyph, because its group's bounding box ended at that glyph's origin. The box now covers every glyph.
