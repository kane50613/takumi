use std::sync::Arc;

use data_url::DataUrl;
use taffy::{AvailableSpace, Layout, Size};

use crate::resources::image::{ImageResult, ImageSource};
use crate::{
  Result,
  layout::{
    inline::InlineContentKind,
    node::{ImageData, Node, NodeKind, NodeStyleLayers},
    style::{Length, Style, StyleDeclaration},
  },
  rendering::{Canvas, RenderContext, draw_image},
  resources::image::{ImageResourceError, is_svg_like},
};

pub(crate) fn image_resource_url(image: &ImageData) -> Option<&str> {
  if image.src.starts_with("https://") || image.src.starts_with("http://") {
    Some(image.src.as_ref())
  } else {
    None
  }
}

pub(crate) fn take_image_style_layers(
  node: &mut Node,
  width: Option<f32>,
  height: Option<f32>,
) -> NodeStyleLayers {
  let mut preset = node.metadata.preset.take();
  if width.is_some() || height.is_some() {
    let preset_style = preset.get_or_insert_with(Style::default);
    if let Some(width) = width {
      preset_style.push(StyleDeclaration::width(Length::Px(width)), false);
    }
    if let Some(height) = height {
      preset_style.push(StyleDeclaration::height(Length::Px(height)), false);
    }
  }

  NodeStyleLayers {
    preset,
    author_tw: node.metadata.tw.take(),
    inline: node.metadata.style.take(),
  }
}

pub(crate) fn image_inline_content(kind: &NodeKind) -> Option<InlineContentKind<'_>> {
  matches!(kind, NodeKind::Image(_)).then_some(InlineContentKind::Box)
}

pub(crate) fn measure_image_node(
  image: &ImageData,
  context: &RenderContext,
  available_space: Size<AvailableSpace>,
  known_dimensions: Size<Option<f32>>,
  style: &taffy::Style,
) -> Size<f32> {
  let Ok(image_source) = resolve_image(&image.src, context) else {
    return Size::zero();
  };

  let intrinsic_size = match &*image_source {
    #[cfg(feature = "svg")]
    ImageSource::Svg { tree, .. } => Size {
      width: tree.size().width(),
      height: tree.size().height(),
    },
    ImageSource::Bitmap(bitmap) => Size {
      width: bitmap.width() as f32,
      height: bitmap.height() as f32,
    },
  };

  let intrinsic_aspect_ratio =
    (intrinsic_size.height != 0.0).then_some(intrinsic_size.width / intrinsic_size.height);
  let preferred_size = match (image.width, image.height) {
    (Some(width), Some(height)) => Size { width, height },
    (Some(width), None) => Size {
      width,
      height: intrinsic_aspect_ratio
        .map(|ratio| width / ratio)
        .unwrap_or(intrinsic_size.height),
    },
    (None, Some(height)) => Size {
      width: intrinsic_aspect_ratio
        .map(|ratio| height * ratio)
        .unwrap_or(intrinsic_size.width),
      height,
    },
    (None, None) => intrinsic_size,
  }
  .map(|value| value * context.sizing.viewport.device_pixel_ratio);

  let style_known_dimensions = Size {
    width: if style.size.width.is_auto() {
      None
    } else {
      match available_space.width {
        AvailableSpace::Definite(width) => Some(width),
        _ => None,
      }
    },
    height: if style.size.height.is_auto() {
      None
    } else {
      match available_space.height {
        AvailableSpace::Definite(height) => Some(height),
        _ => None,
      }
    },
  };

  let known_dimensions = Size {
    width: known_dimensions.width.or(style_known_dimensions.width),
    height: known_dimensions.height.or(style_known_dimensions.height),
  };

  let known_dimensions = if should_skip_intrinsic_probe_cross_axis_ratio_transfer(
    image,
    available_space,
    known_dimensions,
    style,
  ) {
    known_dimensions
  } else {
    let aspect_ratio = style.aspect_ratio.or_else(|| {
      (preferred_size.height != 0.0).then_some(preferred_size.width / preferred_size.height)
    });
    known_dimensions.maybe_apply_aspect_ratio(aspect_ratio)
  };

  if let Size {
    width: Some(width),
    height: Some(height),
  } = known_dimensions
  {
    return Size { width, height };
  }

  preferred_size
}

pub(crate) fn draw_image_node_content(
  image: &ImageData,
  context: &RenderContext,
  canvas: &mut Canvas,
  layout: Layout,
) -> Result<()> {
  let Ok(image_source) = resolve_image(&image.src, context) else {
    return Ok(());
  };

  draw_image(&image_source, context, canvas, layout)?;
  Ok(())
}

fn should_skip_intrinsic_probe_cross_axis_ratio_transfer(
  image: &ImageData,
  available_space: Size<AvailableSpace>,
  known_dimensions: Size<Option<f32>>,
  style: &taffy::Style,
) -> bool {
  image.width.is_none()
    && image.height.is_none()
    && style.size.width.is_auto()
    && style.size.height.is_auto()
    && ((matches!(
      available_space.width,
      AvailableSpace::MinContent | AvailableSpace::MaxContent
    ) && known_dimensions.width.is_none()
      && known_dimensions.height.is_some())
      || (matches!(
        available_space.height,
        AvailableSpace::MinContent | AvailableSpace::MaxContent
      ) && known_dimensions.height.is_none()
        && known_dimensions.width.is_some()))
}

const DATA_URI_PREFIX: &str = "data:";

fn parse_data_uri_image(src: &str) -> ImageResult {
  let url = DataUrl::process(src).map_err(|_| ImageResourceError::InvalidDataUriFormat)?;
  let (data, _) = url
    .decode_to_vec()
    .map_err(|_| ImageResourceError::InvalidDataUriFormat)?;

  ImageSource::from_bytes(&data)
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
    return Ok(img);
  }

  Err(ImageResourceError::Unknown)
}

impl Default for ImageData {
  fn default() -> Self {
    Self {
      src: Arc::<str>::from(""),
      width: None,
      height: None,
    }
  }
}

impl From<&str> for ImageData {
  fn from(src: &str) -> Self {
    Self {
      src: src.into(),
      width: None,
      height: None,
    }
  }
}

impl From<String> for ImageData {
  fn from(src: String) -> Self {
    Self {
      src: src.into(),
      width: None,
      height: None,
    }
  }
}

impl From<Arc<str>> for ImageData {
  fn from(src: Arc<str>) -> Self {
    Self {
      src,
      width: None,
      height: None,
    }
  }
}

impl From<(&str, u32, u32)> for ImageData {
  fn from((src, width, height): (&str, u32, u32)) -> Self {
    Self {
      src: src.into(),
      width: Some(width as f32),
      height: Some(height as f32),
    }
  }
}

impl From<(String, u32, u32)> for ImageData {
  fn from((src, width, height): (String, u32, u32)) -> Self {
    Self {
      src: src.into(),
      width: Some(width as f32),
      height: Some(height as f32),
    }
  }
}

impl From<(Arc<str>, u32, u32)> for ImageData {
  fn from((src, width, height): (Arc<str>, u32, u32)) -> Self {
    Self {
      src,
      width: Some(width as f32),
      height: Some(height as f32),
    }
  }
}

impl From<(&str, f32, f32)> for ImageData {
  fn from((src, width, height): (&str, f32, f32)) -> Self {
    Self {
      src: src.into(),
      width: Some(width),
      height: Some(height),
    }
  }
}

impl From<(String, f32, f32)> for ImageData {
  fn from((src, width, height): (String, f32, f32)) -> Self {
    Self {
      src: src.into(),
      width: Some(width),
      height: Some(height),
    }
  }
}

impl From<(Arc<str>, f32, f32)> for ImageData {
  fn from((src, width, height): (Arc<str>, f32, f32)) -> Self {
    Self {
      src,
      width: Some(width),
      height: Some(height),
    }
  }
}

impl From<(&str, Option<f32>, Option<f32>)> for ImageData {
  fn from((src, width, height): (&str, Option<f32>, Option<f32>)) -> Self {
    Self {
      src: src.into(),
      width,
      height,
    }
  }
}

impl From<(String, Option<f32>, Option<f32>)> for ImageData {
  fn from((src, width, height): (String, Option<f32>, Option<f32>)) -> Self {
    Self {
      src: src.into(),
      width,
      height,
    }
  }
}

impl From<(Arc<str>, Option<f32>, Option<f32>)> for ImageData {
  fn from((src, width, height): (Arc<str>, Option<f32>, Option<f32>)) -> Self {
    Self { src, width, height }
  }
}
