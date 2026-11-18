---
packages:
  takumi-core:
    type: patch
---

# Documents with attributes on most elements take less memory

An element keeps its attributes as a list in place of a map, which took 368 bytes for even one attribute. An 11,000-row PDF whose elements each carry a `data-` attribute peaks at 994 MB of heap in place of 1,073 MB, with the same bytes.
