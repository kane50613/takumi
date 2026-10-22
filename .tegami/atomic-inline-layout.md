---
packages:
  takumi-core: patch
---

# Line up inline blocks and inline images where Chrome does

An inline block now sits on the baseline of its last line as Chrome places it, also when its `height` cuts that line off, and a percentage size inside an inline box resolves against its containing block as Chrome resolves it.
