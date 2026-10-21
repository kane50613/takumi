---
packages:
  takumi-core: patch
---

# Resolve `inherit`, `initial`, and `unset` on custom properties

A custom property set to `inherit`, `initial`, or `unset` now takes the value the keyword names, as in Chrome. `inherit` takes the parent's value, `initial` takes the `@property` initial value or leaves the property unset so `var()` uses its fallback, and `unset` acts as `inherit` or `initial` by whether the property inherits. Before, `var()` substituted the keyword itself, so `--w: inherit; width: var(--w)` became `width: inherit`.
