//! Element and attribute names, after Blink's `SVGNames`.

use phf::phf_map;

macro_rules! names {
  ($(#[$meta:meta])* $name:ident { $($variant:ident => $text:literal),* $(,)? }) => {
    $(#[$meta])*
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[allow(missing_docs)]
    pub enum $name {
      $($variant),*
    }

    impl $name {
      /// The variant named `text`, case-sensitively.
      pub fn from_name(text: &str) -> Option<Self> {
        static MAP: phf::Map<&'static str, $name> = phf_map! { $($text => $name::$variant),* };

        MAP.get(text).copied()
      }

      /// The name as written in markup.
      pub fn name(self) -> &'static str {
        match self {
          $(Self::$variant => $text),*
        }
      }
    }
  };
}

names! {
  /// An SVG element.
  Tag {
    A => "a",
    Circle => "circle",
    ClipPath => "clipPath",
    Defs => "defs",
    Ellipse => "ellipse",
    FeBlend => "feBlend",
    FeColorMatrix => "feColorMatrix",
    FeComponentTransfer => "feComponentTransfer",
    FeComposite => "feComposite",
    FeConvolveMatrix => "feConvolveMatrix",
    FeDiffuseLighting => "feDiffuseLighting",
    FeDisplacementMap => "feDisplacementMap",
    FeDistantLight => "feDistantLight",
    FeDropShadow => "feDropShadow",
    FeFlood => "feFlood",
    FeFuncA => "feFuncA",
    FeFuncB => "feFuncB",
    FeFuncG => "feFuncG",
    FeFuncR => "feFuncR",
    FeGaussianBlur => "feGaussianBlur",
    FeImage => "feImage",
    FeMerge => "feMerge",
    FeMergeNode => "feMergeNode",
    FeMorphology => "feMorphology",
    FeOffset => "feOffset",
    FePointLight => "fePointLight",
    FeSpecularLighting => "feSpecularLighting",
    FeSpotLight => "feSpotLight",
    FeTile => "feTile",
    FeTurbulence => "feTurbulence",
    Filter => "filter",
    G => "g",
    Image => "image",
    Line => "line",
    LinearGradient => "linearGradient",
    Marker => "marker",
    Mask => "mask",
    Path => "path",
    Pattern => "pattern",
    Polygon => "polygon",
    Polyline => "polyline",
    RadialGradient => "radialGradient",
    Rect => "rect",
    Stop => "stop",
    Style => "style",
    Svg => "svg",
    Switch => "switch",
    Symbol => "symbol",
    Text => "text",
    TextPath => "textPath",
    Tref => "tref",
    Tspan => "tspan",
    Use => "use",
  }
}

names! {
  /// An SVG attribute that is not a style property.
  Attr {
    Amplitude => "amplitude",
    Azimuth => "azimuth",
    BaseFrequency => "baseFrequency",
    Bias => "bias",
    ClipPathUnits => "clipPathUnits",
    Cx => "cx",
    Cy => "cy",
    D => "d",
    DiffuseConstant => "diffuseConstant",
    Divisor => "divisor",
    Dx => "dx",
    Dy => "dy",
    EdgeMode => "edgeMode",
    Elevation => "elevation",
    Exponent => "exponent",
    FilterUnits => "filterUnits",
    Fr => "fr",
    Fx => "fx",
    Fy => "fy",
    GradientTransform => "gradientTransform",
    GradientUnits => "gradientUnits",
    Height => "height",
    Href => "href",
    Id => "id",
    In => "in",
    In2 => "in2",
    Intercept => "intercept",
    K1 => "k1",
    K2 => "k2",
    K3 => "k3",
    K4 => "k4",
    KernelMatrix => "kernelMatrix",
    KernelUnitLength => "kernelUnitLength",
    LengthAdjust => "lengthAdjust",
    LimitingConeAngle => "limitingConeAngle",
    MarkerHeight => "markerHeight",
    MarkerUnits => "markerUnits",
    MarkerWidth => "markerWidth",
    MaskContentUnits => "maskContentUnits",
    MaskUnits => "maskUnits",
    Mode => "mode",
    NumOctaves => "numOctaves",
    Offset => "offset",
    Operator => "operator",
    Order => "order",
    Orient => "orient",
    Path => "path",
    PathLength => "pathLength",
    PatternContentUnits => "patternContentUnits",
    PatternTransform => "patternTransform",
    PatternUnits => "patternUnits",
    Points => "points",
    PointsAtX => "pointsAtX",
    PointsAtY => "pointsAtY",
    PointsAtZ => "pointsAtZ",
    PreserveAlpha => "preserveAlpha",
    PreserveAspectRatio => "preserveAspectRatio",
    PrimitiveUnits => "primitiveUnits",
    R => "r",
    Radius => "radius",
    RefX => "refX",
    RefY => "refY",
    RequiredExtensions => "requiredExtensions",
    RequiredFeatures => "requiredFeatures",
    Result => "result",
    Rotate => "rotate",
    Rx => "rx",
    Ry => "ry",
    Scale => "scale",
    Seed => "seed",
    Side => "side",
    Slope => "slope",
    Space => "space",
    SpecularConstant => "specularConstant",
    SpecularExponent => "specularExponent",
    SpreadMethod => "spreadMethod",
    StartOffset => "startOffset",
    StdDeviation => "stdDeviation",
    StitchTiles => "stitchTiles",
    SurfaceScale => "surfaceScale",
    SystemLanguage => "systemLanguage",
    TableValues => "tableValues",
    TargetX => "targetX",
    TargetY => "targetY",
    TextLength => "textLength",
    Type => "type",
    Values => "values",
    ViewBox => "viewBox",
    Width => "width",
    X => "x",
    X1 => "x1",
    X2 => "x2",
    XChannelSelector => "xChannelSelector",
    Y => "y",
    Y1 => "y1",
    Y2 => "y2",
    YChannelSelector => "yChannelSelector",
    Z => "z",
  }
}

names! {
  /// A style property, set by a presentation attribute, a style sheet, or a `style` attribute.
  Property {
    AlignmentBaseline => "alignment-baseline",
    BackgroundColor => "background-color",
    BaselineShift => "baseline-shift",
    ClipPath => "clip-path",
    ClipRule => "clip-rule",
    Color => "color",
    ColorInterpolationFilters => "color-interpolation-filters",
    Display => "display",
    DominantBaseline => "dominant-baseline",
    Fill => "fill",
    FillOpacity => "fill-opacity",
    FillRule => "fill-rule",
    Filter => "filter",
    FloodColor => "flood-color",
    FloodOpacity => "flood-opacity",
    Font => "font",
    FontFamily => "font-family",
    FontKerning => "font-kerning",
    FontOpticalSizing => "font-optical-sizing",
    FontSize => "font-size",
    FontStretch => "font-stretch",
    FontStyle => "font-style",
    FontVariant => "font-variant",
    FontVariationSettings => "font-variation-settings",
    FontWeight => "font-weight",
    ImageRendering => "image-rendering",
    Isolation => "isolation",
    LetterSpacing => "letter-spacing",
    LightingColor => "lighting-color",
    Marker => "marker",
    MarkerEnd => "marker-end",
    MarkerMid => "marker-mid",
    MarkerStart => "marker-start",
    Mask => "mask",
    MaskType => "mask-type",
    MixBlendMode => "mix-blend-mode",
    Opacity => "opacity",
    Overflow => "overflow",
    PaintOrder => "paint-order",
    ShapeRendering => "shape-rendering",
    StopColor => "stop-color",
    StopOpacity => "stop-opacity",
    Stroke => "stroke",
    StrokeDasharray => "stroke-dasharray",
    StrokeDashoffset => "stroke-dashoffset",
    StrokeLinecap => "stroke-linecap",
    StrokeLinejoin => "stroke-linejoin",
    StrokeMiterlimit => "stroke-miterlimit",
    StrokeOpacity => "stroke-opacity",
    StrokeWidth => "stroke-width",
    TextAnchor => "text-anchor",
    TextDecoration => "text-decoration",
    TextRendering => "text-rendering",
    Transform => "transform",
    TransformOrigin => "transform-origin",
    VectorEffect => "vector-effect",
    Visibility => "visibility",
    WhiteSpace => "white-space",
    WordSpacing => "word-spacing",
  }
}

impl Property {
  /// The property a presentation attribute sets, after Blink's
  /// `SVGElement::CssPropertyIdForSVGAttributeName`.
  pub fn from_presentation_attribute(name: &str) -> Option<Self> {
    let property = Self::from_name(name)?;

    matches!(
      property,
      Self::AlignmentBaseline
        | Self::BackgroundColor
        | Self::BaselineShift
        | Self::ClipPath
        | Self::ClipRule
        | Self::Color
        | Self::ColorInterpolationFilters
        | Self::Display
        | Self::DominantBaseline
        | Self::Fill
        | Self::FillOpacity
        | Self::FillRule
        | Self::Filter
        | Self::FloodColor
        | Self::FloodOpacity
        | Self::FontFamily
        | Self::FontSize
        | Self::FontStretch
        | Self::FontStyle
        | Self::FontVariant
        | Self::FontWeight
        | Self::ImageRendering
        | Self::LetterSpacing
        | Self::LightingColor
        | Self::MarkerEnd
        | Self::MarkerMid
        | Self::MarkerStart
        | Self::Mask
        | Self::MaskType
        | Self::Opacity
        | Self::Overflow
        | Self::PaintOrder
        | Self::ShapeRendering
        | Self::StopColor
        | Self::StopOpacity
        | Self::Stroke
        | Self::StrokeDasharray
        | Self::StrokeDashoffset
        | Self::StrokeLinecap
        | Self::StrokeLinejoin
        | Self::StrokeMiterlimit
        | Self::StrokeOpacity
        | Self::StrokeWidth
        | Self::TextAnchor
        | Self::TextDecoration
        | Self::TextRendering
        | Self::Transform
        | Self::TransformOrigin
        | Self::VectorEffect
        | Self::Visibility
        | Self::WordSpacing
    )
    .then_some(property)
  }
}

impl Tag {
  /// Whether the element draws, as a graphics element (SVG 1.1, 1.3).
  pub fn is_graphic(self) -> bool {
    matches!(
      self,
      Self::Circle
        | Self::Ellipse
        | Self::Image
        | Self::Line
        | Self::Path
        | Self::Polygon
        | Self::Polyline
        | Self::Rect
        | Self::Text
        | Self::Use
    )
  }

  /// Whether the element is a gradient.
  pub fn is_gradient(self) -> bool {
    matches!(self, Self::LinearGradient | Self::RadialGradient)
  }

  /// Whether the element is a paint server.
  pub fn is_paint_server(self) -> bool {
    matches!(
      self,
      Self::LinearGradient | Self::RadialGradient | Self::Pattern
    )
  }
}
