---
packages:
  "@takumi-rs/core":
    type: patch
---

### Size `auto` table columns by their content

An all-`auto` table shared its free space evenly, so a one-word column took as
much room as a paragraph. Each column now grows in proportion to its
max-content width, the way Blink distributes it. A table narrower than its
content still approximates.
