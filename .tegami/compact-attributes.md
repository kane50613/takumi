---
packages:
  takumi-core:
    type: patch
---

# Documents with attributes on most elements take less memory

A document whose elements carry attributes, such as `data-` attributes, takes less memory to render, and the output stays the same. An 11,000-row PDF whose elements each carry a `data-` attribute peaks at 994 MB of heap in place of 1,073 MB.
