---
packages:
  "@takumi-rs/core":
    type: patch
---

### Export the CssInput type from @takumi-rs/core

The `css` option referenced `CssInput` without importing it, so type checking failed on the declaration file when `skipLibCheck` was off.
