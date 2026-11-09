---
packages:
  takumi-core: patch
  takumi-pdf: patch
---

# A `break-inside: avoid` box stays on its page when only an absolute box inside it runs past

When an absolutely positioned box inside a `break-inside: avoid` box started past the end of the page, the whole box moved to the next page, even though it fit. It now stays where it fits, and only the absolute box continues on the next page, as Chromium prints it.
