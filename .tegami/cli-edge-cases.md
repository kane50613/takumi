---
packages:
  takumi-core:
    type: minor
  takumi:
    type: minor
  takumi-cli:
    type: patch
---

# A header file can be a whole document, and a page and its header keep their own images

`FromHtmlOptions::fragment` parses a source as a fragment even when it opens with a doctype or `<html>`, dropping the `html`, `head`, and `body` tags as Chromium does with a header template. `takumi pdf` parses `--header` and `--footer` files this way, so a whole HTML file no longer brings its body's margin into the band. A page and its header that both write `<img src="logo.png">` for different files now draw their own image. `--pages 1,-` is refused, an empty `url()` loads nothing, and `--dpr` with an SVG output is an error.
