---
packages:
  takumi-html: minor
---

# Render an `<img>` without a source

An `<img>` with no `src`, or an empty one, used to fail the whole document. It now renders as an empty inline box its `width` and `height` size, as in Chrome, and `HtmlError::MissingImageSrc` is gone.
