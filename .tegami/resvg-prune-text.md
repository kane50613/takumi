---
packages:
  "cargo:takumi-core": patch
---

### Prune the dead text node chain from vendored resvg

Remove `Node::Text`, the text tree types and their render, clip and paint-server arms; the parser already dropped text elements with the text feature stripped.
