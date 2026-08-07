# takumi-svg

<!-- cargo-rdme start -->

Vector SVG output for takumi.

[`render`] turns a node tree into real SVG (`<rect>`, `<path>`,
`<linearGradient>`/`<radialGradient>`, `<filter>`, `<clipPath>`, glyph-outline
`<path>`s, embedded `<image>`) instead of wrapping a rasterized bitmap in a
`data:` URL. [`quick_xml`] builds the document, so every attribute and value
is escaped.

Coverage:

- Backgrounds, borders, border-radius (backgrounds and clip).
- Linear and radial gradients; conic via a wedge-path approximation.
- Box-shadow.
- Text: glyph outlines, decorations, text-shadow, `-webkit-text-stroke`.
- Bitmap and emoji glyphs, images.
- Clip-path, overflow, opacity.
- Filter and backdrop-filter (`<filter>` chains; the backdrop is the scene
  replayed up to the element).
- Affine transforms.

<!-- cargo-rdme end -->
