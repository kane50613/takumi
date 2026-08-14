---
packages:
  takumi-pdf:
    type: patch
---

### Cut once at a forced break

The anonymous box a text child lays out in copied `break-before` and `break-after` from its parent. A padded box carrying `break-after: page` cut twice, once at its content edge and once at its border edge, and the padding between them landed on a blank page of its own.
