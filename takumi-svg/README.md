# takumi-svg

<!-- cargo-rdme start -->

Vector SVG output for takumi.

[`render`] turns a takumi node tree into real SVG (`<rect>`, `<path>`,
`<linearGradient>`/`<radialGradient>`, `<filter>`, `<clipPath>`, glyph outline
`<path>`s, embedded `<image>`) rather than wrapping a rasterized bitmap in a
`data:` URL. The document is built with [`quick_xml`] so every attribute and
value is correctly escaped.

Coverage: backgrounds, borders, border-radius (backgrounds/clip), linear and
radial gradients (conic via a wedge-path approximation), box-shadow, text
(glyph outlines, decorations, text-shadow, `-webkit-text-stroke`), bitmap/
emoji glyphs and images, clip-path/overflow, opacity, and affine transforms.

<!-- cargo-rdme end -->
