---
packages:
  "takumi": patch
---

# Keep decoration lines whole inside opacity and filter layers

Text with `opacity`, `filter` or a blend mode no longer loses the part of an overline, underline or wavy line that reaches past its box. The layer now grows to hold every decoration line, as Chrome counts them in the text's ink overflow.
