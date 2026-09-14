---
"takumi": patch
---

# Report the family a face was registered under from `measure()`

A run's family came from the face's own `name` table, so a font registered under another name reported that other name, and a subsetted file carrying an empty name record reported nothing at all.
