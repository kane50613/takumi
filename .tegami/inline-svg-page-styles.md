---
packages:
  takumi-core: minor
  takumi-html: minor
  takumi: minor
---

# Style inline `<svg>` elements with the page's stylesheet

An inline `<svg>` in HTML now inherits the style of the element it sits in, so `fill`, `stroke`, and font properties set on an ancestor reach its shapes and text. The page's stylesheet also matches the elements inside it, before the `<svg>`'s own `<style>` rules. Selectors that need an HTML ancestor of the `<svg>`, such as `.card path`, still do not match.
