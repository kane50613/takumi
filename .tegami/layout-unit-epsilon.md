---
packages:
  "takumi-core":
    type: patch
---

### Treat layout values a browser reads as equal as equal

Text outline fragments, uniform border widths, and the ellipsis overflow check
all compare against 1/64px now, the step Blink stores layout on. Each used to
carry its own tolerance, so the same difference counted three ways.
