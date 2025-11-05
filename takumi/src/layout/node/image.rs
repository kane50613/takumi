use std::sync::Arc;

use data_url::DataUrl;
use serde::Deserialize;
use taffy::{AvailableSpace, Layout, Size};

use crate::resources::image::{ImageResult, load_image_source_from_bytes};
use crate::{
  layout::{
    inline::InlineContentKind,
    node::Node,
    style::{InheritedStyle, Style, tw::TailwindProperties},
  },
  rendering::{Canvas, RenderContext, draw_image},
  resources::{
    image::{ImageResourceError, ImageSource, is_svg},
    task::FetchTaskCollection,
  },
};

/// A node that renders image content.
#[derive(Debug, Clone, Deserialize)]
pub struct ImageNode {
  /// The styling properties for this image node
  pub style: Option<Style>,
  /// The source URL or path to the image
  pub src: Arc<str>,
  /// The width of the image
  pub width: Option<f32>,
  /// The height of the image
  pub height: Option<f32>,
  /// The tailwind properties for this image node
  pub tw: Option<TailwindProperties>,
}

impl<Nodes: Node<Nodes>> Node<Nodes> for ImageNode {
  fn get_tailwind_properties(&self) -> Option<&TailwindProperties> {
    self.tw.as_ref()
  }

  fn collect_fetch_tasks(&self, collection: &mut FetchTaskCollection) {
    if self.src.starts_with("https://") || self.src.starts_with("http://") {
      collection.insert(self.src.clone());
    }
  }

  fn create_inherited_style(&mut self, parent_style: &InheritedStyle) -> InheritedStyle {
    let mut style = self.style.take().unwrap_or_default();

    if let Some(tw) = self.tw.as_ref() {
      tw.apply(&mut style);
    }

    style.inherit(parent_style)
  }

  fn inline_content(&self, _context: &RenderContext) -> Option<InlineContentKind> {
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
      width: self.width.unwrap_or(image_size.width),
      height: self.height.unwrap_or(image_size.height),
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

  fn draw_content(&self, context: &RenderContext, canvas: &mut Canvas, layout: Layout) {
    let Ok(image) = resolve_image(&self.src, context) else {
      return;
    };

    draw_image(&image, context, canvas, layout);
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

fn resolve_image(src: &str, context: &RenderContext) -> ImageResult {
  if src.starts_with(DATA_URI_PREFIX) {
    return parse_data_uri_image(src);
  }

  if is_svg(src) {
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
