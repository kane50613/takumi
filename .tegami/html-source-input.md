---
packages:
  takumi-html: minor
  "@takumi-rs/core": minor
  "@takumi-rs/wasm": minor
  "@takumi-rs/helpers": minor
---

# Renderers take HTML directly

`render`, `renderSvg`, `measure`, and the WASM `renderAsDataUrl` accept an HTML string in place of a node tree. The markup is parsed in Rust, and the CSS of its `<style>` elements follows the `css` option. `takumi-html` adds `parse_html`, which returns that CSS with the tree. `fromHtml` from `@takumi-rs/helpers/html` is deprecated in favor of passing the string and will be removed in v3.
