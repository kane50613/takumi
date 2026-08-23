---
packages:
  "@takumi-rs/core":
    type: patch
---

### Reuse layout styles across sizing passes

Layout runs several sizing passes over each node, and each pass rebuilt that node's layout style from scratch. Only container query units (`cqw`, `cqh`, `cqmin`, `cqmax`) can change between passes, so a style using none of them is now built once and reused. A deeply nested flex tree lays out about 36% faster.
