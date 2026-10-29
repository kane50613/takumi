# takumi-pdf

<!-- cargo-rdme start -->

Writes vector PDFs from HTML or a takumi node tree, without a browser. Text stays selectable and
searchable because the file embeds a subset of each font it uses. Layout comes from
`takumi-core`, the same engine that renders takumi images.

JavaScript projects use the [`takumi-pdf`](https://www.npmjs.com/package/takumi-pdf) npm
package, which wraps this crate.

## Example

This renders an HTML file to an A4 PDF. It needs `takumi-core` with the `html` feature next to
`takumi-pdf`. Takumi loads no system fonts, so register every font the document uses.

```rust
use std::{error::Error, fs, sync::Arc};

use takumi_core::{
  Fonts,
  html::{FromHtmlOptions, parse_html},
  resources::font::FontResource,
  style::StyleSheet,
};
use takumi_pdf::{PageOptions, PdfOptions, render};

fn main() -> Result<(), Box<dyn Error>> {
  let mut fonts = Fonts::default();
  fonts.register(FontResource::new(fs::read("Geist.woff2")?))?;

  let html = parse_html(&fs::read_to_string("invoice.html")?, FromHtmlOptions::default())?;
  let pdf = render(
    PdfOptions::builder()
      .node(html.node)
      .stylesheet(Arc::new(StyleSheet::parse_owned_list_loosy(html.css)))
      .page(PageOptions::A4)
      .fonts(&fonts)
      .build(),
  )?;

  fs::write("invoice.pdf", pdf)?;
  Ok(())
}
```

[`parse_html`] returns the tree in `node` and the CSS of the document's `<style>` elements in
`css`, which the example passes as the stylesheet.

Without [`PdfOptions::page`], the document is a single page the size of
[`PdfOptions::viewport`]. [`measure`] lays out a tree without writing a PDF, for example to find
how tall a header band is. [`needs`] lists the images and fonts a tree needs fetched before it
renders.

## Pages

With [`PdfOptions::page`] set, content flows across pages as Chromium lays out a printed
document. Lines, boxes, flex and grid items, and table rows break between pages. Content that
cannot break, such as an image, moves to the next page, and continues over the following pages
when it is taller than one.

Pagination honors `break-before: page`, `break-after: page`, `break-inside: avoid`, and the
`widows` and `orphans` minimums, which default to 2 as in Chromium. A table repeats its header
or footer group on every page it crosses when the group has `break-inside: avoid`, which
`<thead>` and `<tfoot>` have by default, and is at most a quarter of the page tall.

[`PdfOptions::header`] and [`PdfOptions::footer`] repeat a band in the top and bottom page
margins, like Chromium's print headers. A band is a node tree, such as the `node` of another
`parse_html` call. Inside a band, takumi fills elements with the class `pageNumber` or
`totalPages` with the current page number or the page count.

## Document features

Besides the pages, [`PdfOptions`] sets what else the file carries:

- Links become link annotations, and [`PdfOptions::outline`] builds an outline from `h1`–`h6`.
- [`PdfOptions::metadata`] writes the document metadata.
- [`PdfOptions::tagged`] writes a structure tree, which is on by default.
- [`PdfOptions::standard`] makes the file conform to PDF/A, and the render fails when the file
  does not validate.
- [`PdfOptions::attachments`] embeds files.
- [`PdfOptions::page_ranges`] keeps only some of the pages.

## What draws

These CSS features draw in the PDF:

- Backgrounds: colors, gradients, and `url()` layers, with `background-size`,
  `background-position`, `background-repeat`, `background-origin`, `background-clip` (including
  `text`), and a blend mode per layer.
- Borders, `border-radius`, `outline`, and `box-shadow`.
- Text with decorations, `text-shadow`, and `-webkit-text-stroke`.
- Images, with `object-fit` and `object-position`.
- `clip-path`, `mask-image`, and the color `filter` functions.
- Opacity, blend modes, overflow clipping, and affine transforms.

Two of these are approximate. Shadow blur is drawn as bands. An inline element fills its
`background-color` for each line separately, as `box-decoration-break: slice` does.
[`InlineBackgroundFragment`] lists where that differs from Chromium.

A PDF has no operator for `filter: blur()` or `drop-shadow()`, so [`render`] fails with
[`PdfError::UnsupportedFilter`] and names the function. `backdrop-filter` is ignored.

[`InlineBackgroundFragment`]: https://docs.rs/takumi-core/latest/takumi_core/layout/inline/struct.InlineBackgroundFragment.html
[`PdfError::UnsupportedFilter`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/enum.PdfError.html#variant.UnsupportedFilter
[`PdfOptions`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html
[`PdfOptions::attachments`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.attachments
[`PdfOptions::footer`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.footer
[`PdfOptions::header`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.header
[`PdfOptions::metadata`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.metadata
[`PdfOptions::outline`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.outline
[`PdfOptions::page`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.page
[`PdfOptions::page_ranges`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.page_ranges
[`PdfOptions::standard`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.standard
[`PdfOptions::tagged`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.tagged
[`PdfOptions::viewport`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/struct.PdfOptions.html#structfield.viewport
[`measure`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/fn.measure.html
[`needs`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/fn.needs.html
[`parse_html`]: https://docs.rs/takumi-core/latest/takumi_core/html/fn.parse_html.html
[`render`]: https://docs.rs/takumi-pdf/latest/takumi_pdf/fn.render.html

<!-- cargo-rdme end -->
