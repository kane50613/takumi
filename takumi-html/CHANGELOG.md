## takumi-html@0.8.0

### Renderers take HTML directly

`render`, `renderSvg`, `measure`, and the WASM `renderAsDataUrl` accept an HTML string in place of a node tree. The markup is parsed in Rust, and the CSS of its `<style>` elements follows the `css` option. A `<style>` with a `media` attribute applies only where its query matches. `takumi-html` adds `parse_html`, which returns that CSS with the tree. `fromHtml` from `@takumi-rs/helpers/html` is deprecated in favor of passing the string and will be removed in v3.

### Draw the value of text fields such as `<input type="text">`

An `<input>` of type `text`, `search`, `tel`, `url`, `email`, `password`, or `number` now draws its `value` inside the field, as Chromium does. The field takes Chromium's default font, border, and padding, sizes itself from the `size` attribute, centers its text vertically, and sits on the baseline of its line. A password shows a dot for each character, a number field with an invalid value shows nothing, and a disabled field turns gray. `<input type="hidden">` takes no space. These inputs used to draw nothing, not even their author-set width.

### `takumi-html` parses with its own parser instead of html5ever

It recovers from less malformed markup than html5ever. It implies `<html>`, `<head>`, `<body>`, `<tbody>`, and the end tags of `p`, `li`, `dt`, `dd`, `option`, and table rows and cells, but does not reconstruct misnested formatting elements or move stray table content out of the table.

### The HTML parser moves into `takumi-core`

`takumi_core::html` holds `from_html`, `parse_html`, `FromHtml`, `FromHtmlOptions`, `StylePresets`, and `HtmlError`, behind the `html` feature. `takumi-html` now only re-exports that module, so depend on `takumi-core` with `features = ["html"]` instead, adding `html-svg-nodes` where `takumi-html`'s `svg` feature kept inline `<svg>` as nodes. The `takumi` crate's `from-html` feature and its `takumi::html` re-export work as before.

### Lay out `<textarea>` as a field sized by `cols` and `rows`

A `<textarea>` now takes Chromium's default monospace font, border, and padding, and sizes itself from `cols` and `rows` with room for a scrollbar. Its text keeps its line breaks and, unless the field is disabled or read-only, its spaces. It wraps inside the field, which clips it in both axes even under `overflow: visible`, as Chromium does. A disabled textarea turns gray. A textarea used to render as inline text with its line breaks collapsed.

## takumi-html@0.7.0

### Style inline `<svg>` elements with the page's stylesheet

An inline `<svg>` in HTML now inherits the style of the element it sits in, so `fill`, `stroke`, and font properties set on an ancestor reach its shapes and text. The page's stylesheet also matches the elements inside it, before the `<svg>`'s own `<style>` rules. Selectors that need an HTML ancestor of the `<svg>`, such as `.card path`, still do not match.

### Lay out tables with Chromium's table algorithm

Tables now follow Chromium's table layout instead of a grid approximation, so column widths, row heights, and collapsed borders match Chromium. `<col>` and `<colgroup>` size their columns and paint their backgrounds, and `visibility: collapse`, `empty-cells: hide`, and `display: inline-table` work. A `rowspan` now stops at the end of its row group.

### Size images by percentage `width` and `height` attributes

`<img width="50%">` and `<svg width="100%" height="2em">` now take those sizes, as Chrome maps the attributes to style. A plain number of pixels sizes an image as before.

## takumi-html@0.6.0

### Parse a document that starts with a byte order mark as a document

`from_html` now drops a leading byte order mark, as a browser does when it decodes a page. A document saved with one used to parse as a fragment, so it lost its `<body>` and the body's margin.

### Keep the box of an element that holds only white space

An element whose only content is white space, such as `<div style="height: 20px; background: green"> </div>`, now takes up space and paints its background and border in `fromHtml`, `fromJsx`, and `takumi-html`. It used to vanish along with its text.

### Parse a source with a doctype as a document

`from_html` parses a source that opens with a doctype as a document even without an `<html>` tag, so it gains the `<html>`, `<head>`, and `<body>` a browser builds, along with the body's default margin.

## takumi-html@0.5.0

### Render an `<img>` without a source

An `<img>` with no `src`, or an empty one, used to fail the whole document. It now renders as an empty inline box its `width` and `height` size, as in Chrome, and `HtmlError::MissingImageSrc` is gone.

### Build the parsed HTML into an arena

`takumi-html` builds the DOM it parses into one `Vec` instead of `markup5ever_rcdom`'s reference-counted nodes, so it no longer depends on `markup5ever_rcdom` or `xml5ever`.

## takumi-html@0.4.2

### Parse repeated inline styles once

`from_html` parses each distinct `style` attribute once per document and reuses it for every element that repeats it. A dashboard of 48 cards with shared styles builds in half the time.

## takumi-html@0.4.0

### Repeat table headers and footers on every page, as Chrome prints them

- A `tfoot` no taller than a quarter of the page repeats below the last row on each page. The structure tree keeps one `TFoot`.
- A header or footer group repeats only with `break-inside: avoid`. The `thead` and `tfoot` presets set it, as Chrome's print stylesheet does.

### Refuse node trees nested deeper than 512 levels

A deeper tree now fails with `nodes nest deeper than 512 levels`, Blink's parser limit, instead of crashing the process. `takumi_html::DEFAULT_MAX_DEPTH` is now `takumi_core::layout::node::MAXIMUM_DOM_TREE_DEPTH`. `takumi::DEFAULT_MAX_DEPTH` keeps its name.

## takumi-html@0.3.0

### Draw list markers for `<ol>` and `<ul>`

List items rendered with no bullet or number. A `display: list-item` box now generates a marker: `list-style-type`, `list-style-position` and `list-style-image` pick what it draws and where it sits, and `<ol start>` and `<li value>` set the count.

## takumi-html@0.2.0

### Keep the `<html>` root when parsing a document

A source starting with `<html>` is parsed as a document, so the tree keeps that element along with `<body>` and the styles on both. It used to be parsed as a fragment, which dropped the wrappers. Anything else is still a fragment and gains no wrappers of its own.

## takumi-html@0.1.24

### Keep the rest of a `style` attribute when one declaration fails

A value this crate cannot read, such as `width: fit-content`, discarded every other declaration in the same `style` attribute. It now invalidates only itself, which is the recovery CSS asks for and what a `<style>` sheet already did.

## takumi-html@0.1.12

### Honour per-element white-space when collapsing inline text

Inline whitespace collapsing read the block's white-space value for every span, so a `white-space: pre` child inside a normal-collapsing parent lost its spaces and line breaks. Each span now collapses against its own value. `<br>` also carries a `white-space: pre` preset, so its line break survives.

## takumi-html@0.1.0

### Build `FromHtmlOptions` with a builder

`FromHtmlOptions` fields are now `pub(crate)` and the struct is
`#[non_exhaustive]`; construct it via `FromHtmlOptions::builder()` (or
`default()`). The `with_presets`, `with_tailwind_property`, and `with_max_depth`
methods are gone.

### Add `takumi-html` for parsing HTML into a node tree

New `takumi-html` crate parses HTML + Tailwind markup into a node tree with
`from_html(source, FromHtmlOptions)`, mirroring the JS `fromHtml`. The `tw`,
`style`, `class`, `id`, `dir`, and `lang` attributes map to node styling and
metadata; `FromHtmlOptions` sets the `StylePresets` table and a `max_depth`
nesting cap. The `takumi` umbrella re-exports it under the `from-html` feature
as `takumi::from_html`, plus `Node::from_html` via the `FromHtml` prelude
trait.

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
