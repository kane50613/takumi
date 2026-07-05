## takumi-html@0.1.0-rc.4

### Build `FromHtmlOptions` with a builder

`FromHtmlOptions` fields are now `pub(crate)` and the struct is
`#[non_exhaustive]`; construct it via `FromHtmlOptions::builder()` (or
`default()`). The `with_presets`, `with_tailwind_property`, and `with_max_depth`
methods are gone.

## takumi-html@0.1.0-rc.2

### Add `takumi-html` for parsing HTML into a node tree

New `takumi-html` crate parses HTML + Tailwind markup into a node tree with
`from_html(source, FromHtmlOptions)`, mirroring the JS `fromHtml`. The `tw`,
`style`, `class`, `id`, `dir`, and `lang` attributes map to node styling and
metadata; `FromHtmlOptions` sets the `StylePresets` table and a `max_depth`
nesting cap. The `takumi` umbrella re-exports it under the `from-html` feature
as `takumi::from_html`, plus `Node::from_html` via the `FromHtml` prelude
trait.
