---
packages:
  "@takumi-rs/helpers":
    type: patch
---

### Keep a newline-wrapped element a single root

`fromHtml` kept the whitespace a template literal leaves around the markup. Those text roots landed in an inline wrapper, where the first one held a line box and pushed the content down the page. They are dropped now, the way the Rust crate's `from_html` already did.
