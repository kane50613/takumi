---
packages:
  takumi-core: minor
---

# Lay out pages one at a time

`PagedLayout` lays a render tree out across pages one `next_page` call at a time, so a caller can read the pages laid out so far before the rest exist.
