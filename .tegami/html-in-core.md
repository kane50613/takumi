---
packages:
  takumi-core: minor
  takumi-html: minor
---

# The HTML parser moves into `takumi-core`

`takumi_core::html` holds `from_html`, `parse_html`, `FromHtml`, `FromHtmlOptions`, `StylePresets`, and `HtmlError`, behind the `html` feature. `takumi-html` now only re-exports that module, so depend on `takumi-core` with `features = ["html"]` instead, adding `html-svg-nodes` where `takumi-html`'s `svg` feature kept inline `<svg>` as nodes. The `takumi` crate's `from-html` feature and its `takumi::html` re-export work as before.
