use std::marker::PhantomData;
use std::num::{NonZeroU16, NonZeroU32};

use smallvec::SmallVec;

use crate::krilla::geom::Rect;
use crate::krilla::surface::Location;

include!("generated.rs");

impl TagKind {
  /// Get the location.
  pub fn location(&self) -> Option<Location> {
    self.as_any().location
  }

  /// Set the location.
  pub fn set_location(&mut self, location: Option<Location>) {
    self.as_any_mut().location = location;
  }

  /// Set the location.
  pub fn with_location(mut self, location: Option<Location>) -> Self {
    self.as_any_mut().location = location;
    self
  }
}

/// A specific tag which allows accessing attributes specific to this [`TagKind`].
///
/// # Example
/// ```ignore
/// use std::num::NonZeroU32;
/// use krilla::tagging::{TagGroup, TagTree, TableHeaderScope, Tag, TagId};
///
/// let tag = Tag::TH(TableHeaderScope::Row)
///     .with_id(Some(TagId::from(*b"this id")))
///     .with_col_span(Some(NonZeroU32::new(3).unwrap()))
///     .with_headers(Some([TagId::from(*b"parent id")]))
///     .with_width(Some(250.0))
///     .with_height(Some(100.0));
/// let group = TagGroup::new(tag);
///
/// let mut tree = TagTree::new();
/// tree.push(group);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Tag<T> {
  inner: AnyTag,
  /// Compile time marker for a type-safe API.
  pub(crate) ty: PhantomData<T>,
}

impl<T> Tag<T> {
  /// This can't be public, otherwise tags could be constructed without
  /// providing required attributes.
  pub(crate) const fn new() -> Self {
    Self {
      inner: AnyTag::new(),
      ty: PhantomData,
    }
  }

  /// A raw tag, which allows reading all attributes.
  pub fn as_any(&self) -> &AnyTag {
    &self.inner
  }

  /// A raw tag, which allows reading all attributes and additionally writing
  /// all global ones.
  pub fn as_any_mut(&mut self) -> &mut AnyTag {
    &mut self.inner
  }

  /// Get the location.
  pub fn location(&self) -> Option<Location> {
    self.as_any().location
  }

  /// Set the location.
  pub fn set_location(&mut self, location: Option<Location>) {
    self.as_any_mut().location = location;
  }

  /// Set the location.
  pub fn with_location(mut self, location: Option<Location>) -> Self {
    self.as_any_mut().location = location;
    self
  }
}

/// A raw tag, which allows reading all attributes and additionally writing all
/// global ones.
#[derive(Clone, Debug, PartialEq)]
pub struct AnyTag {
  /// The location of the tag.
  pub location: Option<Location>,
  pub(crate) attrs: OrdinalSet<Attr>,
}

impl AnyTag {
  pub(crate) const fn new() -> Self {
    Self {
      attrs: OrdinalSet::new(),
      location: None,
    }
  }
}

/// An ordered set using ordinal numbers to sort and identify elements.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct OrdinalSet<A> {
  items: SmallVec<[A; 1]>,
}

impl<A> OrdinalSet<A> {
  pub(crate) const fn new() -> Self {
    Self {
      items: SmallVec::new_const(),
    }
  }
}

impl<A: Ordinal> OrdinalSet<A> {
  pub(crate) fn iter(&self) -> impl Iterator<Item = &A> {
    self.items.iter()
  }

  pub(crate) fn set(&mut self, attr: A) {
    for (i, item) in self.items.iter().enumerate() {
      if item.ordinal() == attr.ordinal() {
        self.items[i] = attr;
        return;
      }
      if item.ordinal() > attr.ordinal() {
        self.items.insert(i, attr);
        return;
      }
    }
    self.items.push(attr);
  }

  pub(crate) fn remove(&mut self, ordinal: usize) {
    for (i, item) in self.items.iter().enumerate() {
      if item.ordinal() == ordinal {
        self.items.remove(i);
        return;
      }
      if item.ordinal() > ordinal {
        break;
      }
    }
  }

  pub(crate) fn get(&self, ordinal: usize) -> Option<&A> {
    for item in self.items.iter() {
      if item.ordinal() == ordinal {
        return Some(item);
      }
      if item.ordinal() > ordinal {
        break;
      }
    }
    None
  }

  pub(crate) fn set_or_remove(&mut self, ordinal: usize, attr: Option<A>) {
    match attr {
      Some(attr) => self.set(attr),
      None => self.remove(ordinal),
    }
  }
}

/// Identifies elements using an ordinal number.
pub(crate) trait Ordinal {
  fn ordinal(&self) -> usize;
}

/// An identifier of a [`Tag`].
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TagId(pub(crate) SmallVec<[u8; 16]>);

impl<I: IntoIterator<Item = u8>> From<I> for TagId {
  fn from(value: I) -> Self {
    // Disambiguate ids provided by the user from ids automatically assigned
    // to notes by prefixing them with a `U`.
    let bytes = std::iter::once(b'U').chain(value).collect();
    TagId(bytes)
  }
}

impl TagId {
  /// Returns the identifier as a byte slice.
  pub fn as_bytes(&self) -> &[u8] {
    self.0.as_slice()
  }
}

/// The list numbering type.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum ListNumbering {
  /// No numbering.
  None,
  /// Solid circular bullets.
  Disc,
  /// Open circular bullets.
  Circle,
  /// Solid square bullets.
  Square,
  /// Decimal numbers.
  Decimal,
  /// Lowercase Roman numerals.
  LowerRoman,
  /// Uppercase Roman numerals.
  UpperRoman,
  /// Lowercase letters.
  LowerAlpha,
  /// Uppercase letters.
  UpperAlpha,
}

impl ListNumbering {
  pub(crate) fn to_pdf(self) -> pdf_writer::types::ListNumbering {
    match self {
      ListNumbering::None => pdf_writer::types::ListNumbering::None,
      ListNumbering::Disc => pdf_writer::types::ListNumbering::Disc,
      ListNumbering::Circle => pdf_writer::types::ListNumbering::Circle,
      ListNumbering::Square => pdf_writer::types::ListNumbering::Square,
      ListNumbering::Decimal => pdf_writer::types::ListNumbering::Decimal,
      ListNumbering::LowerRoman => pdf_writer::types::ListNumbering::LowerRoman,
      ListNumbering::UpperRoman => pdf_writer::types::ListNumbering::UpperRoman,
      ListNumbering::LowerAlpha => pdf_writer::types::ListNumbering::LowerAlpha,
      ListNumbering::UpperAlpha => pdf_writer::types::ListNumbering::UpperAlpha,
    }
  }
}

/// The scope of a table header cell.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum TableHeaderScope {
  /// The header cell refers to the row.
  Row,
  /// The header cell refers to the column.
  Column,
  /// The header cell refers to both the row and the column.
  Both,
}

impl TableHeaderScope {
  pub(crate) fn to_pdf(self) -> pdf_writer::types::TableHeaderScope {
    match self {
      TableHeaderScope::Row => pdf_writer::types::TableHeaderScope::Row,
      TableHeaderScope::Column => pdf_writer::types::TableHeaderScope::Column,
      TableHeaderScope::Both => pdf_writer::types::TableHeaderScope::Both,
    }
  }
}

/// The positioning of the element with respect to the enclosing reference area
/// and other content.
/// When applied to an ILSE, any value except Inline shall cause the element to
/// be treated as a BLSE instead.
///
/// Default value: [`Placement::Inline`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Placement {
  /// Stacked in the block-progression direction within an enclosing reference
  /// area or parent BLSE.
  Block,
  /// Packed in the inline-progression direction within an enclosing BLSE.
  #[default]
  Inline,
  /// Placed so that the before edge of the element’s allocation rectangle.
  /// (see “Content and Allocation Rectangles” in 14.8.5.4, “Layout Attributes”)
  /// coincides with that of the nearest enclosing reference area. The element
  /// may float, if necessary, to achieve the specified placement. The element
  /// shall be treated as a block occupying the full extent of the enclosing
  /// reference area in the inline direction. Other content shall be stacked
  /// so as to begin at the after edge of the element’s allocation rectangle.
  Before,
  /// Placed so that the start edge of the element’s allocation rectangle
  /// (see “Content and Allocation Rectangles” in 14.8.5.4, “Layout Attributes”)
  /// coincides with that of the nearest enclosing reference area. The element
  /// may float, if necessary, to achieve the specified placement. Other
  /// content that would intrude into the element’s allocation rectangle
  /// shall be laid out as a runaround.
  Start,
  /// Placed so that the end edge of the element’s allocation rectangle
  /// (see “Content and Allocation Rectangles” in 14.8.5.4, “Layout Attributes”)
  /// coincides with that of the nearest enclosing reference area. The element
  /// may float, if necessary, to achieve the specified placement. Other
  /// content that would intrude into the element’s allocation rectangle
  /// shall be laid out as a runaround.
  End,
}

impl Placement {
  pub(crate) fn to_pdf(self) -> pdf_writer::types::Placement {
    match self {
      Placement::Block => pdf_writer::types::Placement::Block,
      Placement::Inline => pdf_writer::types::Placement::Inline,
      Placement::Before => pdf_writer::types::Placement::Before,
      Placement::Start => pdf_writer::types::Placement::Start,
      Placement::End => pdf_writer::types::Placement::End,
    }
  }
}

/// The directions of layout progression for packing of ILSEs (inline progression)
/// and stacking of BLSEs (block progression).
/// The specified layout directions shall apply to the given structure element
/// and all of its descendants to any level of nesting.
///
/// Default value: [`WritingMode::LrTb`].
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WritingMode {
  /// Inline progression from left to right; block progression from top to
  /// bottom. This is the typical writing mode for Western writing systems.
  #[default]
  LrTb,
  /// Inline progression from right to left; block progression from top to
  /// bottom. This is the typical writing mode for Arabic and Hebrew writing
  /// systems.
  RlTb,
  /// Inline progression from top to bottom; block progression from right to
  /// left. This is the typical writing mode for Chinese and Japanese writing
  /// systems.
  TbRl,
}

impl WritingMode {
  pub(crate) fn to_pdf(self) -> pdf_writer::types::WritingMode {
    match self {
      WritingMode::LrTb => pdf_writer::types::WritingMode::LtrTtb,
      WritingMode::RlTb => pdf_writer::types::WritingMode::RtlTtb,
      WritingMode::TbRl => pdf_writer::types::WritingMode::TtbRtl,
    }
  }
}

/// The bounding box of a tag that encloses its visible content.
/// If the content spans multiple pages, this should be omitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BBox {
  /// The page index of the bounding box.
  pub page_idx: usize,
  /// The rectangle that encloses the content.
  pub rect: Rect,
}

impl BBox {
  /// Create a new bounding box.
  pub fn new(page_idx: usize, rect: Rect) -> Self {
    Self { page_idx, rect }
  }
}

/// An RGB color within the tag tree. The color space of this color is not
/// specified. Each component is in the range `0..=255`; see [`Self::new_f32`]
/// for `[0.0, 1.0]` float input.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NaiveRgbColor {
  /// The red component of the color.
  pub red: u8,
  /// The green component of the color.
  pub green: u8,
  /// The blue component of the color.
  pub blue: u8,
}

impl NaiveRgbColor {
  /// Create a new RGB color.
  pub fn new(red: u8, green: u8, blue: u8) -> Self {
    Self { red, green, blue }
  }

  /// Create a new RGB color from normalized floating point values.
  pub fn new_f32(red: f32, green: f32, blue: f32) -> Self {
    if !(0.0..=1.0).contains(&red) || !(0.0..=1.0).contains(&green) || !(0.0..=1.0).contains(&blue)
    {
      panic!("RGB color components must be in the range [0.0, 1.0]");
    }
    Self {
      red: (255.0 * red).round() as u8,
      green: (255.0 * green).round() as u8,
      blue: (255.0 * blue).round() as u8,
    }
  }

  /// Convert the color into an array of f32 components for PDF serialization.
  pub fn into_f32_array(self) -> [f32; 3] {
    let normalize = |n| n as f32 / 255.0;
    [self.red, self.green, self.blue].map(normalize)
  }
}

impl From<NaiveRgbColor> for crate::krilla::graphics::color::rgb::Color {
  fn from(color: NaiveRgbColor) -> Self {
    crate::krilla::graphics::color::rgb::Color::new(color.red, color.green, color.blue)
  }
}

impl From<NaiveRgbColor> for [f32; 3] {
  fn from(color: NaiveRgbColor) -> Self {
    color.into_f32_array()
  }
}

/// The border style of an element.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum BorderStyle {
  /// No border.
  None,
  /// Hidden border.
  Hidden,
  /// Solid border.
  Solid,
  /// Dashed border.
  Dashed,
  /// Dotted border.
  Dotted,
  /// Double border.
  Double,
  /// Groove border.
  Groove,
  /// Ridge border.
  Ridge,
  /// Inset border.
  Inset,
  /// Outset border.
  Outset,
}

impl BorderStyle {
  pub(super) fn to_pdf(self) -> pdf_writer::types::LayoutBorderStyle {
    match self {
      BorderStyle::None => pdf_writer::types::LayoutBorderStyle::None,
      BorderStyle::Hidden => pdf_writer::types::LayoutBorderStyle::Hidden,
      BorderStyle::Solid => pdf_writer::types::LayoutBorderStyle::Solid,
      BorderStyle::Dashed => pdf_writer::types::LayoutBorderStyle::Dashed,
      BorderStyle::Dotted => pdf_writer::types::LayoutBorderStyle::Dotted,
      BorderStyle::Double => pdf_writer::types::LayoutBorderStyle::Double,
      BorderStyle::Groove => pdf_writer::types::LayoutBorderStyle::Groove,
      BorderStyle::Ridge => pdf_writer::types::LayoutBorderStyle::Ridge,
      BorderStyle::Inset => pdf_writer::types::LayoutBorderStyle::Inset,
      BorderStyle::Outset => pdf_writer::types::LayoutBorderStyle::Outset,
    }
  }
}

/// The text alignment.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum TextAlign {
  /// At the start of the inline advance direction.
  Start,
  /// Centered.
  Center,
  /// At the end of the inline advance direction.
  End,
  /// Justified.
  Justify,
}

impl TextAlign {
  pub(super) fn to_pdf(self) -> pdf_writer::types::TextAlign {
    match self {
      TextAlign::Start => pdf_writer::types::TextAlign::Start,
      TextAlign::Center => pdf_writer::types::TextAlign::Center,
      TextAlign::End => pdf_writer::types::TextAlign::End,
      TextAlign::Justify => pdf_writer::types::TextAlign::Justify,
    }
  }
}

/// The block alignment.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum BlockAlign {
  /// At the start of the block advance direction.
  Begin,
  /// Centered.
  Middle,
  /// At the end of the block advance direction.
  After,
  /// Justified.
  Justify,
}

impl BlockAlign {
  pub(super) fn to_pdf(self) -> pdf_writer::types::BlockAlign {
    match self {
      BlockAlign::Begin => pdf_writer::types::BlockAlign::Before,
      BlockAlign::Middle => pdf_writer::types::BlockAlign::Middle,
      BlockAlign::After => pdf_writer::types::BlockAlign::After,
      BlockAlign::Justify => pdf_writer::types::BlockAlign::Justify,
    }
  }
}

/// The inline alignment.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum InlineAlign {
  /// At the start of the inline advance direction.
  Start,
  /// Centered.
  Center,
  /// At the end of the inline advance direction.
  End,
}

impl InlineAlign {
  pub(super) fn to_pdf(self) -> pdf_writer::types::InlineAlign {
    match self {
      InlineAlign::Start => pdf_writer::types::InlineAlign::Start,
      InlineAlign::Center => pdf_writer::types::InlineAlign::Center,
      InlineAlign::End => pdf_writer::types::InlineAlign::End,
    }
  }
}

/// The height of a line.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum LineHeight {
  /// Adjust the line height automatically, taking `/BaselineShift` into
  /// account.
  Normal,
  /// Adjust the line height automatically.
  Auto,
  /// Set a fixed line height.
  Custom(f32),
}

impl LineHeight {
  pub(super) fn to_pdf(self) -> pdf_writer::types::LineHeight {
    match self {
      LineHeight::Auto => pdf_writer::types::LineHeight::Auto,
      LineHeight::Normal => pdf_writer::types::LineHeight::Normal,
      LineHeight::Custom(height) => pdf_writer::types::LineHeight::Custom(height),
    }
  }
}

/// The text decoration type (over- and underlines).
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum TextDecorationType {
  /// No decoration.
  None,
  /// Underlined.
  Underline,
  /// Line over the text.
  Overline,
  /// Strike the text.
  LineThrough,
}

impl TextDecorationType {
  pub(super) fn to_pdf(self) -> pdf_writer::types::TextDecorationType {
    match self {
      Self::None => pdf_writer::types::TextDecorationType::None,
      Self::Underline => pdf_writer::types::TextDecorationType::Underline,
      Self::Overline => pdf_writer::types::TextDecorationType::Overline,
      Self::LineThrough => pdf_writer::types::TextDecorationType::LineThrough,
    }
  }
}

/// The rotation of glyphs in vertical writing modes.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum GlyphOrientationVertical {
  /// Determine the rotation based on whether the text is full-width.
  Auto,
  /// No rotation.
  None,
  /// Rotate 90 degrees clockwise.
  Clockwise90,
  /// Rotate 90 degrees counter-clockwise.
  CounterClockwise90,
  /// Rotate 180 degrees clockwise.
  Clockwise180,
  /// Rotate 180 degrees counter-clockwise.
  CounterClockwise180,
  /// Rotate 270 degrees clockwise.
  Clockwise270,
}

impl GlyphOrientationVertical {
  /// Convert the rotation to a number. If the rotation is `Auto`, returns
  /// `None`.
  pub(super) fn to_pdf(self) -> pdf_writer::types::GlyphOrientationVertical {
    let angle = match self {
      GlyphOrientationVertical::Auto => return pdf_writer::types::GlyphOrientationVertical::Auto,
      GlyphOrientationVertical::None => 0,
      GlyphOrientationVertical::Clockwise90 => 90,
      GlyphOrientationVertical::CounterClockwise90 => -90,
      GlyphOrientationVertical::Clockwise180 => 180,
      GlyphOrientationVertical::CounterClockwise180 => -180,
      GlyphOrientationVertical::Clockwise270 => 270,
    };
    pdf_writer::types::GlyphOrientationVertical::Angle(angle)
  }
}

/// An attribute value that can apply to all sides of the element, or have a specific value for each side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sides<T> {
  /// The start of the element on the block axis.
  pub before: T,
  /// The end of the element on the block axis.
  pub after: T,
  /// The start of the element on the inline axis.
  pub start: T,
  /// The end of the element on the inline axis.
  pub end: T,
}

impl<T> Sides<T> {
  /// Construct a new `Sides` value with specific values for each side.
  pub fn new(before: T, after: T, start: T, end: T) -> Self {
    Self {
      before,
      after,
      start,
      end,
    }
  }

  /// Construct a new `Sides` value with the same value for all sides.
  pub fn uniform(value: T) -> Self
  where
    T: Copy,
  {
    Sides {
      before: value,
      after: value,
      start: value,
      end: value,
    }
  }

  pub(crate) fn is_uniform(&self) -> bool
  where
    T: PartialEq,
  {
    self.before == self.after && self.before == self.start && self.before == self.end
  }

  /// Returns an array for all sides.
  pub(super) fn into_array(self) -> [T; 4] {
    [self.before, self.after, self.start, self.end]
  }

  /// Convert into [`pdf_writer::types::Sides`].
  pub(super) fn into_pdf(self) -> pdf_writer::types::Sides<T> {
    pdf_writer::types::Sides::from_array(self.into_array())
  }

  /// Convert into [`pdf_writer::types::Sides`] by each side value.
  pub(super) fn map_pdf<P>(self, to_pdf: impl Fn(T) -> P) -> pdf_writer::types::Sides<P> {
    pdf_writer::types::Sides::from_array(self.into_array().map(to_pdf))
  }
}

/// Widths related to columns, either for all columns or
/// with specific values for each.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnDimensions {
  /// The same value applies to all columns.
  All(f32),
  /// The value varies for each column or column gap.
  Specific(Vec<f32>),
}

impl ColumnDimensions {
  /// Construct a new `ColumnDimensions` with the same value for all columns.
  pub fn all(value: f32) -> Self {
    ColumnDimensions::All(value)
  }

  /// Construct a new `ColumnDimensions` with specific values for each column.
  pub fn specific(values: Vec<f32>) -> Self {
    ColumnDimensions::Specific(values)
  }
}
