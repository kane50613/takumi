---
packages:
  takumi-core:
    type: patch
---

### Apply text-fit to text with letter or word spacing

`text-fit` shrink and grow now scale text that sets `letter-spacing` or
`word-spacing`. Both used to disable the fit silently, so a spaced headline
overflowed instead of shrinking.
