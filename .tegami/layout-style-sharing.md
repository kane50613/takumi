---
packages:
  takumi-core:
    type: patch
  takumi-pdf:
    type: patch
---

# Long documents with repeated styles take less memory

A document that repeats the same layout styles, such as a table whose rows share theirs, takes less memory to render, and the output stays the same. An 11,000-row PDF peaks at 939 MB of heap in place of 1,073 MB.
