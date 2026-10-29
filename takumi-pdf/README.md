# takumi-pdf

<!-- cargo-rdme start -->

Vector PDF output for takumi. The `takumi-pdf` npm package wraps this crate.

[`render`] lays out the tree with `takumi-core` and walks the same stacking-context scene as
`takumi-svg`. A vendored krilla fork writes the PDF. Backgrounds become filled paths. Text
becomes glyph runs in embedded subset fonts, so it stays selectable and searchable.
[`measure`] lays out a tree without writing a PDF, such as to size a header band.

## Pagination

With [`PdfOptions::page`] set, content lays out on pages through takumi-core's block
fragmentation, as Chromium lays out a printed document. Boxes, lines, flex and grid items,
and table rows break across pages. Content that cannot break, such as an image, moves to the
next page, and runs on over the pages after it when it is taller than one. Each page paints
from the scene of the pages that reach it, as soon as no page still to lay out can.

Pagination honors `break-before: page`, `break-after: page`, `break-inside: avoid`, and the
`widows` and `orphans` minimums, which default to 2 as in Chromium. A table header or footer
group with `break-inside: avoid`, the default for `<thead>` and `<tfoot>`, repeats on each
page the table crosses if it is at most a quarter of the page tall. The header and footer
bands repeat in the page margins, like Chromium's print templates. Nodes classed `pageNumber`
or `totalPages` receive the page counters.

## Document features

- Link annotations, and an outline built from `h1`–`h6` with [`PdfOptions::outline`].
- Document metadata through [`PdfOptions::metadata`].
- A structure tree, on by default, set by [`PdfOptions::tagged`].
- PDF/A conformance through [`PdfOptions::standard`]. A validation failure fails the render.
- File attachments through [`PdfOptions::attachments`].
- A subset of pages through [`PdfOptions::page_ranges`].

## CSS coverage

- backgrounds: color, gradient and url() layers, with `background-size`,
  `-position`, `-repeat`, `-origin`, `-clip` (including `text`) and
  per-layer blend modes; an inline span fills `background-color` per line
  fragment, `box-decoration-break: slice` style (the drifts are listed on
  [`takumi_core::layout::inline::InlineBackgroundFragment`])
- borders, border-radius, `outline`, and `box-shadow`
- text with decorations, `text-shadow`, and `-webkit-text-stroke`
- shadow blur, approximated by bands
- images, with `object-fit` and `object-position`
- `clip-path`, `mask-image`, and the color `filter` functions
- opacity, blend modes, overflow clipping, and affine transforms

A PDF has no operator for `filter: blur()` or `drop-shadow()`. [`render`] fails with
[`PdfError::UnsupportedFilter`], naming the function. `backdrop-filter` is not drawn.

<!-- cargo-rdme end -->
