---
packages:
  "takumi": patch
---

# Skip unknown at-rules in `StyleSheet::parse`

`StyleSheet::parse` used to fail the whole sheet on an at-rule it does not implement, such as `@font-face`, `@charset`, or `@page`. It now drops that at-rule and keeps the rest, as browsers do. `parse_loosy` already behaved this way.
