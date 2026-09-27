---
packages:
  "takumi": patch
  "takumi-pdf": patch
---

# Drop collapsible spaces at the end of a paragraph

Text that ends a block, such as `<span>Label </span>` inside a flex row, no longer keeps its trailing space in its width, as browsers remove it.
