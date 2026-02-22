use std::sync::Arc;

use data_url::DataUrl;
use serde::Deserialize;
use taffy::{AvailableSpace, Layout, Size};

use crate::layout::Viewport;
use crate::resources::image::{ImageResult, load_image_source_from_bytes};
use crate::{
  Result,
  layout::{
    inline::InlineContentKind,
    node::Node,
    style::{ComputedStyle, Style, tw::TailwindValues},
  },
  rendering::{Canvas, RenderContext, draw_image},
  resources::{
    image::{ImageResourceError, ImageSource, is_svg_like},
    task::FetchTaskCollection,
  },
};

/// A node that renders image content.
#[derive(Debug, Clone, Deserialize)]
pub struct ImageNode {
  /// Default style presets from HTML element type (lowest priority)
  pub preset: Option<Style>,
  /// The styling properties for this image node
  pub style: Option<Style>,
  /// The source URL or path to the image
  pub src: Arc<str>,
  /// The width of the image
  pub width: Option<f32>,
  /// The height of the image
  pub height: Option<f32>,
  /// The tailwind properties for this image node
  pub tw: Option<TailwindValues>,
}

impl<Nodes: Node<Nodes>> Node<Nodes> for ImageNode {
  fn collect_fetch_tasks(&self, collection: &mut FetchTaskCollection) {
    if self.src.starts_with("https://") || self.src.starts_with("http://") {
      collection.insert(self.src.clone());
    }
  }

  fn compute_style(&mut self, parent_style: &ComputedStyle, viewport: Viewport) -> ComputedStyle {
    // Start with empty style
    let mut style = Style::default();

    // 1. Apply preset first (lowest priority)
    if let Some(preset) = self.preset.take() {
      style.merge_from(preset);
    }

    // 2. Apply Tailwind (medium priority)
    if let Some(tw) = self.tw.as_ref() {
      tw.apply(&mut style, viewport);
    }

    // 3. Merge inline style last (highest priority)
    if let Some(inline_style) = self.style.take() {
      style.merge_from(inline_style);
    }

    style.compute_style(parent_style)
  }

  fn inline_content(&self) -> Option<InlineContentKind<'_>> {
    Some(InlineContentKind::Box)
  }

  fn measure(
    &self,
    context: &RenderContext,
    _available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
    style: &taffy::Style,
  ) -> Size<f32> {
    let Ok(image) = resolve_image(&self.src, context) else {
      return Size::zero();
    };

    let image_size = match &*image {
      #[cfg(feature = "svg")]
      ImageSource::Svg(svg) => Size {
        width: svg.size().width(),
        height: svg.size().height(),
      },
      ImageSource::Bitmap(bitmap) => Size {
        width: bitmap.width() as f32,
        height: bitmap.height() as f32,
      },
    };

    let overridden_size = Size {
      width: self.width.unwrap_or(image_size.width) * context.sizing.viewport.device_pixel_ratio,
      height: self.height.unwrap_or(image_size.height) * context.sizing.viewport.device_pixel_ratio,
    };

    let aspect_ratio = style
      .aspect_ratio
      .unwrap_or(overridden_size.width / overridden_size.height);

    if let Size {
      width: Some(width),
      height: Some(height),
    } = known_dimensions.maybe_apply_aspect_ratio(Some(aspect_ratio))
    {
      return Size { width, height };
    }

    overridden_size
  }

  fn draw_content(
    &self,
    context: &RenderContext,
    canvas: &mut Canvas,
    layout: Layout,
  ) -> Result<()> {
    let Ok(image) = resolve_image(&self.src, context) else {
      return Ok(());
    };

    draw_image(&image, context, canvas, layout)?;
    Ok(())
  }

  fn get_style(&self) -> Option<&Style> {
    self.style.as_ref()
  }
}

const DATA_URI_PREFIX: &str = "data:";

fn parse_data_uri_image(src: &str) -> ImageResult {
  let url = DataUrl::process(src).map_err(|_| ImageResourceError::InvalidDataUriFormat)?;
  let (data, _) = url
    .decode_to_vec()
    .map_err(|_| ImageResourceError::InvalidDataUriFormat)?;

  load_image_source_from_bytes(&data)
}

pub(crate) fn resolve_image(src: &str, context: &RenderContext) -> ImageResult {
  if src.starts_with(DATA_URI_PREFIX) {
    return parse_data_uri_image(src);
  }

  if is_svg_like(src) {
    #[cfg(feature = "svg")]
    return crate::resources::image::parse_svg_str(src);
    #[cfg(not(feature = "svg"))]
    return Err(ImageResourceError::SvgParseNotSupported);
  }

  if let Some(img) = context.fetched_resources.get(src) {
    return Ok(img.clone());
  }

  if let Some(img) = context.global.persistent_image_store.get(src) {
    return Ok(img.clone());
  }

  Err(ImageResourceError::Unknown)
}
