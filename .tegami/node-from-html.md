---
packages:
  cargo:takumi-html:
    replay:
      - "exit prerelease: cargo:takumi-html"
  cargo:takumi:
    replay:
      - "exit prerelease: cargo:takumi"
---

### Add `takumi-html` for parsing HTML into a node tree

The new `takumi-html` crate turns HTML + Tailwind markup into a node tree via
`from_html(source, FromHtmlOptions)`, mirroring the JS `fromHtml` helper. `tw`,
`style`, `class`, `id`, `dir`, and `lang` attributes map to node styling and
metadata. `FromHtmlOptions` enables the built-in `StylePresets::chromium`
presets, a custom preset table, or none. Exposed through the `takumi` umbrella
under the `from-html` feature as `takumi::from_html` and, via the `FromHtml`
prelude trait, `Node::from_html`.
