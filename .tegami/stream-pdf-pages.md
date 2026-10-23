---
packages:
  takumi-pdf: patch
---

# Compose PDF pages while later pages still lay out

`render` draws a page as soon as no page still to lay out can paint onto it, so a long document holds one page's scene at a time. Layout errors, including `TooManyPages`, can now arrive after earlier pages were composed, so an error from drawing a page can come before one from laying out a later page.
