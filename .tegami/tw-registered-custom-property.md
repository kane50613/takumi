---
packages:
  "takumi": patch
---

# Keep the registered value of a `--tw-*` custom property

The utility engine's `--tw-*` state stops at the element that sets it, but that applied to every such name. An `@property` rule registering one lost its initial value, so Tailwind's compiled `linear-gradient(..., var(--tw-gradient-from-position))` painted nothing.
