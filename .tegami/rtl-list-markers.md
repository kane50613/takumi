---
packages:
  "@takumi-rs/core":
    type: patch
---

### Render list markers on the right under `direction: rtl`

A `direction: rtl` list item now places its marker at the right edge and mirrors the counter suffix, matching Chrome. RTL blocks also force the right-to-left base direction instead of inferring it from the first strong character, so their text aligns to the right regardless of content.
