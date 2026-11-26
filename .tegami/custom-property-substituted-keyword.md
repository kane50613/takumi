---
packages:
  takumi-core: patch
---

# CSS-wide keywords that `var()` substitutes into a custom property

`--a: var(--none, inherit)` now inherits `--a` as in Chrome, and `initial` and `unset` apply the same way, where only the revert keywords did.
