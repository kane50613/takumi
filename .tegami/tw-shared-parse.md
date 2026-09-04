---
packages:
  takumi-core:
    type: minor
---

### Share a parsed class list between nodes

`TailwindValues::interned` returns the parsed form of a class list from a shared cache, and `Node::with_tw_source` sets a node's utilities from that source string. Nodes carrying the same classes now hold one parsed value.
