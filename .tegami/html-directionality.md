---
packages:
  takumi-core: patch
  "@takumi-rs/helpers": patch
---

# `dir="auto"` takes one direction from its text, as in Chrome

An element with `dir="auto"`, and a `<bdi>`, now takes its `direction` and `:dir()` from the first strong character of its text. It skips `<bdi>`, `<script>`, `<style>`, `<textarea>`, and descendants with a `dir` of `ltr`, `rtl`, or `auto`. Before, each paragraph inside it took its own direction, and `:dir()` read its parent's. A tel input keeps `ltr` under an `rtl` parent.
