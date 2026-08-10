---
packages:
  "@takumi-rs/helpers":
    type: patch
---

### Collect `<head>` styles and decode character references in `fromHtml`

`fromHtml` skipped the whole `<head>` subtree, so a full document lost every `<style>` rule while the same markup as a fragment kept them. `<style>` tags inside `<head>` now land in `stylesheets` for both `fromHtml` and `fromJsx`. Text nodes also decode HTML character references, so `&nbsp;`, `&deg;` and `&#176;` render as the characters instead of the raw source.
