---
packages:
  "takumi": patch
---

# Inherit a `--tw-`prefixed variable the utility engine never wrote

A custom property was dropped from inheritance whenever its name started with `--tw-`. The utility engine now registers the state it writes, so only that state stops at its element and an author's own `--tw-` name inherits like any other.
