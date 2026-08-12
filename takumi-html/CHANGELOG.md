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
