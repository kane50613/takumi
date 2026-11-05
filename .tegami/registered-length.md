---
packages:
  takumi-core: patch
---

# A registered `<length>` custom property inherits as px

A custom property registered with `@property`, `syntax: "<length>"` and `inherits: true` now computes to px on the element that sets it, and its descendants inherit that px value. `--g: .5em` set at a 20px font size is 10px in every descendant, as browsers compute it, where each child used to read `.5em` again at its own font size.
