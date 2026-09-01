---
packages:
  "takumi-core":
    type: patch
---

### Break lines where Chromium breaks them

Soft wrap opportunities follow Chromium's table instead of plain ICU, so
`ISBN-2026408` wraps after the hyphen the way a browser wraps it. Text under
`word-break: break-all` still breaks at every character.
