---
packages:
  "@takumi-rs/helpers":
    type: patch
  "@takumi-rs/core":
    type: patch
---

### Treat the synthetic HTML root as a block container

Markup written in a template literal carries whitespace either side, which parsed into text roots. The synthetic root that holds them was inline, so the leading one kept a line box and pushed the content down the page. It is a block container now, the way `<body>` is, and `fromHtml` drops the whitespace roots the way the Rust crate already did.
