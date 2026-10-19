# takumi-svg

<!-- cargo-rdme start -->

Vector SVG output for takumi.

[`render`] turns a node tree into SVG elements such as `<rect>`, `<path>`, `<linearGradient>`,
`<radialGradient>`, `<filter>`, `<clipPath>`, `<mask>`, glyph outline `<path>`s, and
embedded `<image>`s. It does not wrap a rasterized bitmap in a `data:` URL. [`quick_xml`]
builds the document, so every attribute and value is escaped.

In the `takumi` crate, enable the `svg-backend` feature and call `takumi::render_svg`.

Coverage:

- Backgrounds, borders, border-radius (backgrounds and clip).
- Linear and radial gradients; conic via a wedge-path approximation.
- Box-shadow.
- Text covers glyph outlines, decorations, text-shadow, and `-webkit-text-stroke`.
- Bitmap and emoji glyphs, images.
- Clip-path, `mask-image`, overflow, opacity, and blend modes.
- Filter and backdrop-filter (`<filter>` chains; the backdrop is the scene
  replayed up to the element).
- Affine transforms.

<!-- cargo-rdme end -->
