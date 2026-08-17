---
packages:
  "@takumi-rs/core":
    type: minor
---

### Share inherited font lists across nodes

`font-family`, `font-variation-settings`, and `font-feature-settings` lists are now `Arc`-shared, so inheriting them no longer copies the list per node.
