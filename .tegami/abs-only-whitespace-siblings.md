---
packages:
  "cargo:takumi-core": patch
---

### Drop whitespace between absolute-only block siblings

When every element child of a block container was absolutely positioned, the whitespace text nodes from pretty-printed HTML formed an inline formatting context that swallowed the out-of-flow boxes, so none of them rendered. The whitespace drop now also runs when the only in-flow content is whitespace, keeping the absolute children in the layout.
