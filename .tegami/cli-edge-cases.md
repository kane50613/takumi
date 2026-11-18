---
packages:
  takumi-core:
    type: minor
  takumi:
    type: minor
  takumi-cli:
    type: patch
---

# `takumi pdf` takes a whole HTML file as a header or footer

A `--header` or `--footer` file that opens with a doctype or `<html>` now fills the band with its body's content, as Chromium does with a header template. Before, the file brought its body's margin into the band.

- A page and its header that both write `<img src="logo.png">` for different files now each draw their own image.
- `takumi pdf --pages 1,-` is refused with "`-` names no page. Write a page on at least one side of `-`."
- An empty `url()` in a style sheet loads nothing.
- `takumi image --dpr` with an `.svg` output is an error, since an SVG has no pixels to scale.

In Rust, `FromHtmlOptions::fragment` parses a source as a fragment even when it opens with a doctype or `<html>`, dropping its `html`, `head`, and `body` tags.

```rust
let options = FromHtmlOptions::builder().fragment(true).build();
// The node is the <p> alone
let node = parse_html("<!doctype html><html><body><p>hi</p></body></html>", options)?.node;
```
