---
packages:
  takumi-core: patch
---

# Unset a property whose `var()` substitutes to nothing valid

A declaration such as `width: var(--missing)` now resets the property as `unset` does when the variable is missing or substitutes to an invalid value, as in Chrome. It used to be skipped, so an earlier value for the same property stayed in effect.
