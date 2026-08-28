---
packages:
  "@takumi-rs/core":
    type: patch
---

### Take an inline-block's baseline from the lines it actually laid out

An inline-block with horizontal padding re-wrapped its content against the
border box to find its baseline, so text beside it aligned with the wrong line.
