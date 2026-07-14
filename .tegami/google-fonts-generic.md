---
packages:
  "npm:@takumi-rs/helpers": minor
---

### Support generic on googleFonts families

A family's `generic` (e.g. `"monospace"`) propagates to every loaded coverage subset, so generic stacks like `font-mono` resolve to it.
