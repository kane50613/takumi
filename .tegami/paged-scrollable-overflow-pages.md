---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# Content moved past the last page gets pages of its own

A box that a transform or `position: relative` moved past the end of the content now adds the pages it reaches, as Chromium prints it. Before, the document ended on the last page of the content, and the moved box was cut off. A shadow, and content its box clips, add no page, as in Chromium.
