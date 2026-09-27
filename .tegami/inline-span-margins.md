---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Reserve the horizontal margins of inline spans

A `display: inline` element's left and right margins now push the text beside it apart, on the parent's background, as browsers lay them out. Negative margins still reserve nothing.
