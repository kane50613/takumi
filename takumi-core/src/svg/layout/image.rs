//! The `image` element, after Blink's `LayoutSVGImage` and `SVGImagePainter`: a bitmap or a
//! nested SVG, fitted into its rectangle by `preserveAspectRatio`.

#[cfg(feature = "gif")]
use std::sync::Arc;
use std::{mem, rc::Rc};

use super::{
  Scope, Styled, SvgLayout,
  object::{Container, LayoutClip, LayoutDocument, LayoutObject, Picture, PictureContent},
  view_box::PreserveAspectRatio,
};
#[cfg(feature = "gif")]
use crate::resources::image_decoder::{decode_gif_frames, gif_dimensions};
use crate::{
  geometry::{Rect, Size},
  resources::{
    image::{DecodedDataUri, SvgImageData, decode_data_uri},
    image_decoder::{DetectedImageFormat, bitmap_dimensions, detect_image_format},
  },
  style::{Affine, ImageScalingAlgorithm, Length, Size as SizeValue, Visibility},
  svg::{SvgDocument, length::LengthAxis},
};

/// How deep SVG images may nest in SVG images.
const MAXIMUM_IMAGE_DEPTH: usize = 8;

/// What an `href` holds.
enum Source {
  Bitmap(Vec<u8>),
  Gif(Vec<u8>),
  Svg(Vec<u8>),
}

impl SvgLayout<'_> {
  pub(super) fn convert_image(&mut self, styled: &Styled, scope: &Scope, parent: &mut Container) {
    let _ = self.lay_out_image(styled, scope, parent);
  }

  fn lay_out_image(
    &mut self,
    styled: &Styled,
    scope: &Scope,
    parent: &mut Container,
  ) -> Option<()> {
    let (content, actual_size) = self.cached_image_content(styled.index)?;
    let style = styled.style();
    let geometry = &style.svg_geometry_data;
    let x = self.px(styled, scope, geometry.x, LengthAxis::X);
    let y = self.px(styled, scope, geometry.y, LengthAxis::Y);
    let specified = |size: SizeValue| match size {
      SizeValue::Length(length) if !matches!(length, Length::Auto) => Some(length),
      _ => None,
    };
    let specified_width = specified(style.box_data.width);
    let specified_height = specified(style.box_data.height);
    let mut width = specified_width.map_or(actual_size.width, |length| {
      self.px(styled, scope, length, LengthAxis::X)
    });
    let mut height = specified_height.map_or(actual_size.height, |length| {
      self.px(styled, scope, length, LengthAxis::Y)
    });

    match (specified_width.is_some(), specified_height.is_some()) {
      // Only the width is given, so the height follows the ratio of the image.
      (true, false) => height = actual_size.height * (width / actual_size.width),
      (false, true) => width = actual_size.width * (height / actual_size.height),
      _ => {}
    }

    let aspect = self
      .attribute(styled.index, "preserveAspectRatio")
      .and_then(PreserveAspectRatio::parse)
      .unwrap_or_default();
    let rect = Rect::positive_from_xywh(x, y, width, height)?;
    let rendering = match style.misc_inherited_data.image_rendering {
      ImageScalingAlgorithm::Pixelated => ImageScalingAlgorithm::Pixelated,
      _ => ImageScalingAlgorithm::Auto,
    };
    let picture = Picture {
      content,
      width: actual_size.width,
      height: actual_size.height,
      visible: style.inherited_data.visibility == Visibility::Visible,
      rendering,
    };

    Self::fit_image(picture, aspect, actual_size, rect, parent)
  }

  /// Puts the picture into `parent` scaled and aligned into `rect`, and cropped to it when the
  /// aspect ratio slices.
  fn fit_image(
    picture: Picture,
    aspect: PreserveAspectRatio,
    actual_size: Size<f32>,
    rect: Rect<f32>,
    parent: &mut Container,
  ) -> Option<()> {
    let target = Size::new(rect.width(), rect.height());
    let aligned_size = if aspect.stretches() {
      target
    } else {
      actual_size.fitted_to(target, aspect.slice)
    };
    let (aligned_x, aligned_y) = aspect.aligned_position(
      rect.x(),
      rect.y(),
      rect.width() - aligned_size.width,
      rect.height() - aligned_size.height,
    );
    let view_box = Rect::positive_from_xywh(
      aligned_x,
      aligned_y,
      aligned_size.width,
      aligned_size.height,
    )?;
    let image_transform = Affine {
      a: view_box.width() / actual_size.width,
      b: 0.0,
      c: 0.0,
      d: view_box.height() / actual_size.height,
      x: view_box.x(),
      y: view_box.y(),
    };
    let mut group = Container::new(parent.abs_transform * image_transform);

    group.transform = image_transform;
    group
      .children
      .push(LayoutObject::Picture(Box::new(picture)));
    group.finish();

    if aspect.slice {
      // A slice crops to the rectangle, which the image transform must not move.
      let mut cropped = Container::new(parent.abs_transform);

      cropped.clip_path = Some(Rc::new(LayoutClip {
        transform: Affine::IDENTITY,
        clip_path: None,
        root: Container::filled_rectangle(rect)?,
      }));
      cropped
        .children
        .push(LayoutObject::Container(Box::new(group)));
      cropped.finish();
      parent
        .children
        .push(LayoutObject::Container(Box::new(cropped)));
    } else {
      parent
        .children
        .push(LayoutObject::Container(Box::new(group)));
    }

    Some(())
  }

  /// What the `image` at `index` shows, decoded once however many `use` elements instantiate it.
  fn cached_image_content(&mut self, index: usize) -> Option<(PictureContent, Size<f32>)> {
    if let Some(cached) = self.images.get(&index) {
      return cached.clone();
    }

    let content = self
      .attribute(index, "href")
      .and_then(|href| self.image_content(href));

    self.images.insert(index, content.clone());
    content
  }

  /// What an `image` shows, and its natural size. Only `data:` URLs load, as files and the network
  /// are not the image's to read.
  fn image_content(&mut self, href: &str) -> Option<(PictureContent, Size<f32>)> {
    if !self.budget.spend_image_bytes(href.len()) {
      return None;
    }

    let DecodedDataUri { mime, bytes: data } = decode_data_uri(href).ok()?;
    let source = match mime.as_str() {
      "image/jpg" | "image/jpeg" | "image/png" | "image/webp" => Source::Bitmap(data),
      "image/gif" => Source::Gif(data),
      "image/svg+xml" => Source::Svg(data),
      "text/plain" => match detect_image_format(&data) {
        Some(DetectedImageFormat::Gif) => Source::Gif(data),
        Some(_) => Source::Bitmap(data),
        None => Source::Svg(data),
      },
      _ => return None,
    };

    match source {
      Source::Bitmap(data) => {
        let (width, height) = bitmap_dimensions(&data)?.ok()?;
        let size = positive_size(width as f32, height as f32)?;

        Some((
          PictureContent::Bitmap(Some(Rc::new(SvgImageData::Encoded(data.into())))),
          size,
        ))
      }
      Source::Gif(data) => self.gif_content(data),
      Source::Svg(data) => {
        let document = self.nested_document(&data)?;
        let size = positive_size(document.width, document.height)?;

        Some((PictureContent::Svg(Rc::new(document)), size))
      }
    }
  }

  #[cfg(feature = "gif")]
  fn gif_content(&mut self, data: Vec<u8>) -> Option<(PictureContent, Size<f32>)> {
    let (width, height) = gif_dimensions(&data).ok()?;
    let size = positive_size(width as f32, height as f32)?;

    // A frame decodes to four bytes a pixel.
    if !self.budget.spend_image_bytes(
      (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4),
    ) {
      return None;
    }

    let mut first = None;

    let decoded = decode_gif_frames(&data, 0, Some(1), None, |frame| first = Some(frame)).is_ok();
    let image = first
      .filter(|_| decoded)
      .map(|frame| Rc::new(SvgImageData::Bitmap(Arc::unwrap_or_clone(frame))));

    Some((PictureContent::Bitmap(image), size))
  }

  #[cfg(not(feature = "gif"))]
  fn gif_content(&mut self, _data: Vec<u8>) -> Option<(PictureContent, Size<f32>)> {
    None
  }

  /// A nested SVG image laid out, none when it does not parse, nests too deep, or needs a renderer
  /// this pipeline lacks.
  fn nested_document(&mut self, data: &[u8]) -> Option<LayoutDocument> {
    if self.image_depth >= MAXIMUM_IMAGE_DEPTH {
      return None;
    }

    let document = SvgDocument::parse(str::from_utf8(data).ok()?).ok()?;

    if document.needs_legacy_renderer() {
      self.unsupported = true;
      return None;
    }

    let current_color = self.styles.root_context().current_color;
    let mut layout = SvgLayout::nested(&document, current_color, self.fonts, self.image_depth + 1);

    layout.budget = mem::take(&mut self.budget);

    let nested = layout.layout_root();

    self.budget = mem::take(&mut layout.budget);
    self.unsupported |= layout.unsupported;
    let nested = nested?;
    Some(nested)
  }
}

fn positive_size(width: f32, height: f32) -> Option<Size<f32>> {
  (width > 0.0 && width.is_finite() && height > 0.0 && height.is_finite())
    .then_some(Size::new(width, height))
}
