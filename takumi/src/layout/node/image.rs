use std::{collections::BTreeMap, sync::Arc};

use data_url::DataUrl;
use serde::Deserialize;
use taffy::{AvailableSpace, Layout, Size};

use crate::resources::image::{ImageResult, load_image_source_from_bytes};
use crate::{
  Result,
  layout::{
    inline::InlineContentKind,
    node::{Node, NodeMetadata, NodeStyleLayers},
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
  /// Shared node metadata.
  #[serde(flatten)]
  pub(crate) metadata: NodeMetadata,
  /// The source URL or path to the image
  pub(crate) src: Arc<str>,
  /// The width of the image
  pub(crate) width: Option<f32>,
  /// The height of the image
  pub(crate) height: Option<f32>,
}

impl ImageNode {
  /// Set the tag name and return the updated image node.
  pub fn with_tag_name(mut self, tag_name: impl Into<Box<str>>) -> Self {
    self.metadata.tag_name = Some(tag_name.into());
    self
  }

  /// Set the class name and return the updated image node.
  pub fn with_class_name(mut self, class_name: impl Into<Box<str>>) -> Self {
    self.metadata.class_name = Some(class_name.into());
    self
  }

  /// Set the id and return the updated image node.
  pub fn with_id(mut self, id: impl Into<Box<str>>) -> Self {
    self.metadata.id = Some(id.into());
    self
  }

  /// Set the attributes and return the updated image node.
  pub fn with_attributes(mut self, attributes: BTreeMap<Box<str>, Box<str>>) -> Self {
    self.metadata.attributes = Some(attributes);
    self
  }

  /// Set the preset style and return the updated image node.
  pub fn with_preset(mut self, preset: Style) -> Self {
    self.metadata.preset = Some(preset);
    self
  }

  /// Set the inline style and return the updated image node.
  pub fn with_style(mut self, style: Style) -> Self {
    self.metadata.style = Some(style);
    self
  }

  /// Set the Tailwind values and return the updated image node.
  pub fn with_tw(mut self, tw: TailwindValues) -> Self {
    self.metadata.tw = Some(tw);
    self
  }

  /// Set the source URL or path to the image.
  pub fn with_src(mut self, src: impl Into<Arc<str>>) -> Self {
    self.src = src.into();
    self
  }

  /// Set the width of the image.
  pub fn with_width(mut self, width: f32) -> Self {
    self.width = Some(width);
    self
  }

  /// Set the height of the image.
  pub fn with_height(mut self, height: f32) -> Self {
    self.height = Some(height);
    self
  }
}

impl<Nodes: Node<Nodes>> Node<Nodes> for ImageNode {
  fn metadata(&self) -> &NodeMetadata {
    &self.metadata
  }

  fn metadata_mut(&mut self) -> &mut NodeMetadata {
    &mut self.metadata
  }

  fn collect_fetch_tasks(&self, collection: &mut FetchTaskCollection) {
    if self.src.starts_with("https://") || self.src.starts_with("http://") {
      collection.insert(self.src.clone());
    }
  }

  fn get_preset(&self) -> Option<&Style> {
    self.metadata.preset.as_ref()
  }

  fn get_style(&self) -> Option<&Style> {
    self.metadata.style.as_ref()
  }

  fn take_style_layers(&mut self) -> NodeStyleLayers {
    let mut preset = self.metadata.preset.take();
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
      author_tw: self.metadata.tw.take(),
      inline: self.metadata.style.take(),
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
