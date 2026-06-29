---
packages:
  cargo:takumi-core:
    replay:
      - "exit prerelease: cargo:takumi-core"
  cargo:takumi:
    replay:
      - "exit prerelease: cargo:takumi"
---

### Add `Node::from_html` to parse HTML into a node tree

Behind the `from_html` feature, `Node::from_html(source, FromHtmlOptions)` turns
HTML + Tailwind markup into a node tree, mirroring the JS `fromHtml` helper.
`tw`/`style`/`class` attributes map to node styling and `FromHtmlOptions` exposes
the Chromium element presets for override or opt-out.
