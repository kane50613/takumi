---
packages:
  "@takumi-rs/core":
    type: patch
---

### Cut allocations in style property parsing

Parsing a string-valued property copied the value before handing it to cssparser. Normalizing a kebab-case or camelCase property name allocated a second string just to trim leading underscores. String values now parse in place, and names normalize in one allocation.
