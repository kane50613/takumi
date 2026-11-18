---
packages:
  takumi-core: minor
  takumi: minor
---

# `parse_html` reports the style sheets a document links

`ParsedHtml` now lists each `<link rel="stylesheet">` in `links` and the first `<base href>` in `base_href`, so a host can fetch linked CSS and resolve it as a browser does. `ParsedHtml::css_with_links` takes a fetch function and returns the CSS of every `<style>` element and fetched link in document order, wrapping a link's `media` in an `@media` rule. Titled and alternate style sheets follow the preferred style sheet set, as in Chrome, so only the first set's `<style>` elements stay in `css`.
