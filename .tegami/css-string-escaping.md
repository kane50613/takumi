---
packages:
  "@takumi-rs/core":
    type: patch
---

### Escape control characters in serialized CSS strings

A quoted string containing a newline left it unescaped. This produced invalid CSS. CSS strings now use cssparser's escaping.
