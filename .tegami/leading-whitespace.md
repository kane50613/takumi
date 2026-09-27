---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Drop collapsible spaces at the start of a paragraph

Text that opens a block, such as indented HTML or a `::after` item that starts with a space, no longer keeps a leading space, as browsers remove it.
