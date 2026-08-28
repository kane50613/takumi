---
packages:
  "@takumi-rs/core":
    type: minor
---

### Reject trailing content in `style` values

A `style` object accepted `width: "55px zzz"`, applied `55px`, and ignored the rest. It now rejects the whole value, the way a stylesheet already drops such a declaration. A substituted `var()` value follows the same rule.
