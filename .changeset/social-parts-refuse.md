---
"@takumi-rs/core": major
"@takumi-rs/wasm": major
"takumi": major
---

**Changed initial `display` value from `flex` to `inline`**

This is to comply with [the CSSWG spec](https://drafts.csswg.org/css-display/#the-display-properties).

You should update your code to use `display: flex` if you want to use flexbox.
