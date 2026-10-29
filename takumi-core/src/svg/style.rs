//! Style recalc for an SVG document, after Blink's `StyleResolver`.

use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::Arc};

use super::{SvgDocument, SvgHost};
use crate::{
  context::RenderContext,
  geometry::Size,
  layout::node::Node,
  matching::{NodeMatchedDeclarations, match_stylesheets_view},
  resources::font::Fonts,
  style::{ColorInput, ComputedStyle, LonghandId, SizingContext},
  viewport::Viewport,
};

/// What Blink's `LayoutSVGRoot` paints as an inline `svg`'s box. Approximate: the page's styles
/// win over the document's.
const ROOT_BOX_EFFECTS: [LonghandId; 8] = [
  LonghandId::BackgroundColor,
  LonghandId::Opacity,
  LonghandId::Transform,
  LonghandId::Isolation,
  LonghandId::MixBlendMode,
  LonghandId::ClipPath,
  LonghandId::Filter,
  LonghandId::MaskImage,
];

/// Resolves the style of the elements of one document.
pub(super) struct StyleResolver<'a> {
  document: &'a SvgDocument,
  nodes: Vec<&'a Node>,
  matched: Vec<NodeMatchedDeclarations<'a>>,
  root_context: RenderContext,
  /// Whether the document is an inline `svg`, whose root the box it is drawn in already paints.
  inline: bool,
  /// The contexts of elements resolved where they stand in the document.
  in_place: RefCell<HashMap<usize, RenderContext>>,
}

impl<'a> StyleResolver<'a> {
  /// A resolver whose root inherits from `host`, laid out at `viewport`, which the viewport units
  /// of an image measure.
  pub(super) fn new(document: &'a SvgDocument, host: SvgHost<'a>, viewport: Size<f32>) -> Self {
    let (root_context, matched, inline) = match host {
      SvgHost::Image {
        current_color,
        fonts,
      } => {
        let viewport = Viewport::new((
          viewport.width.round() as u32,
          viewport.height.round() as u32,
        ));
        let mut parent_style = ComputedStyle::default();

        parent_style.inherited_data_mut().color = ColorInput::Value(current_color);

        let root_context = RenderContext::builder()
          .fonts(
            fonts
              .cloned()
              .unwrap_or_else(|| Fonts::default().snapshot()),
          )
          .sizing(SizingContext::builder().viewport(viewport).build())
          .current_color(current_color)
          .style(parent_style)
          .stylesheet(Arc::clone(&document.stylesheet))
          .build();

        (
          root_context,
          match_stylesheets_view(&document.root, &[&document.stylesheet], viewport),
          false,
        )
      }
      SvgHost::Inline(host) => {
        let page = host.stylesheet();
        let root_context = RenderContext::builder()
          .fonts(host.fonts().clone())
          .sizing(host.sizing.clone())
          .current_color(host.current_color)
          .style(Rc::clone(&host.style))
          .stylesheet(Arc::clone(page))
          .time_ms(host.time_ms())
          .build();

        (
          root_context,
          match_stylesheets_view(
            &document.root,
            &[page, &document.stylesheet],
            host.sizing.viewport,
          ),
          true,
        )
      }
    };

    Self {
      document,
      nodes: document.nodes(),
      matched,
      root_context,
      inline,
      in_place: RefCell::default(),
    }
  }

  /// The context the root element inherits from.
  pub(super) fn root_context(&self) -> &RenderContext {
    &self.root_context
  }

  /// The node of the element at `index`.
  pub(super) fn node(&self, index: usize) -> &'a Node {
    self.nodes[index]
  }

  /// The context of the element at `index` inheriting from `parent`.
  pub(super) fn resolve(&self, index: usize, parent: &RenderContext) -> RenderContext {
    let mut shell = self.nodes[index].styling_clone();
    let (mut style, sizing, current_color) =
      parent.resolve_child_style(&mut shell, index, &self.matched);

    if self.inline && index == 0 {
      let host = &self.root_context.style;

      for property in ROOT_BOX_EFFECTS {
        if !host.is_initial_longhand(property) {
          Rc::make_mut(&mut style).reset_longhand(property);
        }
      }
    }

    RenderContext::from_parent(parent, style, sizing, current_color)
  }

  /// The context of the element at `index` where it stands in the document, inheriting along its
  /// ancestors. Resources such as gradients and markers style their content this way.
  pub(super) fn in_place(&self, index: usize) -> RenderContext {
    if let Some(context) = self.in_place.borrow().get(&index) {
      return context.clone();
    }

    let context = match self.document.elements[index].parent {
      Some(parent) => self.resolve(index, &self.in_place(parent)),
      None => self.resolve(index, &self.root_context),
    };

    self.in_place.borrow_mut().insert(index, context.clone());
    context
  }
}
