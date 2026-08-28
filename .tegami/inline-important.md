---
packages:
  "@takumi-rs/core":
    type: patch
---

### Honor `!important` in inline styles

An inline `!important` declaration now outranks important rules and animations. Both the `style` object and an HTML `style` attribute read the marker.
