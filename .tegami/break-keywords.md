---
packages:
  takumi-core: minor
  takumi-pdf: patch
---

# Parse every `break-before`, `break-after`, and `break-inside` keyword

`break-before` and `break-after` now accept `avoid`, `avoid-page`, `avoid-column`, `column`, `recto`, and `verso`, and keep `left` and `right` as their own values. `break-inside` accepts `avoid-page` and `avoid-column`. In a PDF, `recto` and `verso` force a page break, and `avoid-page` keeps a box on one page as `avoid` does.
