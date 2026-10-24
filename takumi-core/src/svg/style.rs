//! Style recalc for an SVG document, after Blink's `StyleResolver`: every element resolves
//! against the style of the element it inherits from, which is its parent in the document and
//! the `use` element for an instance of it.

use std::{cell::RefCell, collections::HashMap, sync::Arc};

use super::SvgDocument;
use crate::{
  context::RenderContext,
  layout::node::Node,
  matching::{NodeMatchedDeclarations, match_stylesheets_view},
  resources::font::{Fonts, FontsSnapshot},
  style::{Color, ColorInput, ComputedStyle, SizingContext},
  viewport::Viewport,
};

/// Resolves the style of the elements of one document.
pub(super) struct StyleResolver<'a> {
  document: &'a SvgDocument,
  nodes: Vec<&'a Node>,
  matched: Vec<NodeMatchedDeclarations<'a>>,
  root_context: RenderContext,
  /// The contexts of elements resolved where they stand in the document.
  in_place: RefCell<HashMap<usize, RenderContext>>,
}

impl<'a> StyleResolver<'a> {
  /// A resolver whose root inherits `current_color` as its `color`, as an SVG image inherits the
  /// `color` of the box it sits in.
  pub(super) fn new(
    document: &'a SvgDocument,
    current_color: Color,
    fonts: Option<&FontsSnapshot>,
  ) -> Self {
    let size = document.size;
    let viewport = Viewport::new((size.width.round() as u32, size.height.round() as u32));
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

    Self {
      document,
      nodes: document.nodes(),
      matched: match_stylesheets_view(&document.root, &document.stylesheet, viewport),
      root_context,
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
    let (style, sizing, current_color) =
      parent.resolve_child_style(&mut shell, index, &self.matched);

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
