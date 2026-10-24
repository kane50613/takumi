// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::resources::font::Fonts;
use crate::svg::tree::{ImageHrefResolver, ImageRendering, ShapeRendering, TextRendering};

/// Processing options.
pub struct Options<'a> {
  /// Directory that will be used during relative paths resolving.
  ///
  /// Expected to be the same as the directory that contains the SVG file,
  /// but can be set to any.
  ///
  /// Default: `None`
  pub resources_dir: Option<std::path::PathBuf>,

  /// Target DPI.
  ///
  /// Impacts units conversion.
  ///
  /// Default: 96.0
  pub dpi: f32,

  /// A default font size.
  ///
  /// Will be used when no `font-size` attribute is set in the SVG.
  ///
  /// Default: 12
  pub font_size: f32,

  /// A list of languages.
  ///
  /// Will be used to resolve a `systemLanguage` conditional attribute.
  ///
  /// Format: en, en-US.
  ///
  /// Default: `[en]`
  pub languages: Vec<String>,

  /// Specifies the default shape rendering method.
  ///
  /// Will be used when an SVG element's `shape-rendering` property is set to `auto`.
  ///
  /// Default: GeometricPrecision
  pub shape_rendering: ShapeRendering,

  /// Specifies the default text rendering method.
  ///
  /// Will be used when an SVG element's `text-rendering` property is set to `auto`.
  ///
  /// Default: OptimizeLegibility
  pub text_rendering: TextRendering,

  /// Specifies the default image rendering method.
  ///
  /// Will be used when an SVG element's `image-rendering` property is set to `auto`.
  ///
  /// Default: OptimizeQuality
  pub image_rendering: ImageRendering,

  /// Specifies the way `xlink:href` in `<image>` elements should be handled.
  ///
  /// Default: see type's documentation for details
  pub image_href_resolver: ImageHrefResolver<'a>,

  /// The fonts `<text>` matches and shapes with.
  pub fonts: Fonts,

  /// A CSS stylesheet that should be injected into the SVG. Can be used to overwrite
  /// certain attributes.
  pub style_sheet: Option<String>,

  /// `currentColor` fallback for when no `color` attribute is set on the
  /// element or its ancestors, mirroring how an inline SVG inherits `color`
  /// from its host document.
  ///
  /// Default: `None` (resolves to black)
  pub current_color: Option<crate::style::Color>,

  /// Set by the parser when a `currentColor` fell through to the
  /// [`current_color`](Self::current_color) fallback, meaning the rendered
  /// tree depends on the host color.
  pub current_color_used: std::sync::atomic::AtomicBool,
}

impl Default for Options<'_> {
  fn default() -> Options<'static> {
    Options {
      resources_dir: None,
      dpi: 96.0,
      font_size: 12.0,
      languages: vec!["en".to_string()],
      shape_rendering: ShapeRendering::default(),
      text_rendering: TextRendering::default(),
      image_rendering: ImageRendering::default(),
      image_href_resolver: ImageHrefResolver::default(),
      fonts: Fonts::default(),
      style_sheet: None,
      current_color: None,
      current_color_used: std::sync::atomic::AtomicBool::new(false),
    }
  }
}

impl Options<'_> {
  /// Converts a relative path into absolute relative to the SVG file itself.
  ///
  /// If `Options::resources_dir` is not set, returns itself.
  pub fn get_abs_path(&self, rel_path: &std::path::Path) -> std::path::PathBuf {
    match self.resources_dir {
      Some(ref dir) => dir.join(rel_path),
      None => rel_path.into(),
    }
  }
}
