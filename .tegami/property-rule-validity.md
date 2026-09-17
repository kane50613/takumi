---
packages:
  "takumi": patch
---

# Drop an `@property` rule that cannot fall back

`@property --size { syntax: "<length>"; inherits: false; }` was kept even though a typed syntax has no value to fall back to without `initial-value`. The rule is now ignored, so the name behaves as the ordinary custom property it is. `syntax` also loses the quotes it was stored with.
