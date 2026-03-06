use std::sync::Arc;

use data_url::DataUrl;
use serde::Deserialize;
use taffy::{AvailableSpace, Layout, Size};

use crate::resources::image::{ImageResult, load_image_source_from_bytes};
use crate::{
  Result,
  layout::{
    inline::InlineContentKind,
    node::{Node, NodeStyleLayers},
    style::{Length, Style, StyleDeclaration, tw::TailwindValues},
  },
  rendering::{Canvas, RenderContext, draw_image},
  resources::{
    image::{ImageResourceError, ImageSource, is_svg_like},
    task::FetchTaskCollection,
  },
};

/// A node that renders image content.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImageNode {
  /// The element's tag name
  pub tag_name: Option<Box<str>>,
  /// The element's class name
  pub class_name: Option<Box<str>>,
  /// The element's id
  pub id: Option<Box<str>>,
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
  fn tag_name(&self) -> Option<&str> {
    self.tag_name.as_deref()
  }

  fn class_name(&self) -> Option<&str> {
    self.class_name.as_deref()
  }

  fn id(&self) -> Option<&str> {
    self.id.as_deref()
  }

  fn collect_fetch_tasks(&self, collection: &mut FetchTaskCollection) {
    if self.src.starts_with("https://") || self.src.starts_with("http://") {
      collection.insert(self.src.clone());
    }
  }

  fn take_style_layers(&mut self) -> NodeStyleLayers {
    let mut preset = self.preset.take();
    if self.width.is_some() || self.height.is_some() {
      let preset_style = preset.get_or_insert_with(Style::default);
      if let Some(width) = self.width {
        preset_style.push(StyleDeclaration::width(Length::Px(width)), false);
      }
      if let Some(height) = self.height {
        preset_style.push(StyleDeclaration::height(Length::Px(height)), false);
      }
    }

    NodeStyleLayers {
      preset,
      author_tw: self.tw.take(),
      inline: self.style.take(),
    }
  }

  fn inline_content(&self) -> Option<InlineContentKind<'_>> {
    Some(InlineContentKind::Box)
  }

  fn measure(
    &self,
    context: &RenderContext,
    available_space: Size<AvailableSpace>,
    known_dimensions: Size<Option<f32>>,
    style: &taffy::Style,
  ) -> Size<f32> {
    let Ok(image) = resolve_image(&self.src, context) else {
      return Size::zero();
    };

    let intrinsic_size = match &*image {
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
    let preferred_size = match (self.width, self.height) {
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
      self,
      available_space,
      known_dimensions,
      style,
    ) {
      // During flex min/max-content probing, a stretched cross-size should not
      // determine this replaced element's intrinsic main-size.
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

  fn is_replaced_element(&self) -> bool {
    true
  }
}

fn should_skip_intrinsic_probe_cross_axis_ratio_transfer(
  node: &ImageNode,
  available_space: Size<AvailableSpace>,
  known_dimensions: Size<Option<f32>>,
  style: &taffy::Style,
) -> bool {
  node.width.is_none()
    && node.height.is_none()
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
