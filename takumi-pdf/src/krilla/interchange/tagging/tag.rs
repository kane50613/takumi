use std::num::{NonZeroU16, NonZeroU32};

use pdf_writer::types::{
  ListNumbering as PdfListNumbering, TableHeaderScope as PdfTableHeaderScope,
};
use smallvec::SmallVec;

use crate::krilla::geom::Rect;
use crate::krilla::surface::Location;

#[derive(Clone, Debug, PartialEq)]
pub struct Tag {
  pub(crate) location: Option<Location>,
  pub(crate) kind: TagKind,
  pub(crate) attrs: OrdinalSet<Attr>,
}

impl Tag {
  pub const ARTICLE: Self = Self::plain(TagKind::Article);
  pub const SECTION: Self = Self::plain(TagKind::Section);
  pub const BLOCK_QUOTE: Self = Self::plain(TagKind::BlockQuote);
  pub const CAPTION: Self = Self::plain(TagKind::Caption);
  pub const P: Self = Self::plain(TagKind::P);
  pub const LI: Self = Self::plain(TagKind::LI);
  pub const LBL: Self = Self::plain(TagKind::Lbl);
  pub const L_BODY: Self = Self::plain(TagKind::LBody);
  pub const TABLE: Self = Self::plain(TagKind::Table);
  pub const TR: Self = Self::plain(TagKind::TR);
  pub const T_HEAD: Self = Self::plain(TagKind::THead);
  pub const T_BODY: Self = Self::plain(TagKind::TBody);
  pub const T_FOOT: Self = Self::plain(TagKind::TFoot);
  pub const SPAN: Self = Self::plain(TagKind::Span);
  pub const CODE: Self = Self::plain(TagKind::Code);
  pub const LINK: Self = Self::plain(TagKind::Link);
  pub const STRONG: Self = Self::plain(TagKind::Strong);
  pub const EM: Self = Self::plain(TagKind::Em);

  const fn plain(kind: TagKind) -> Self {
    Self {
      location: None,
      kind,
      attrs: OrdinalSet::new(),
    }
  }

  pub fn heading(level: NonZeroU16, title: Option<String>) -> Self {
    let mut tag = Self::plain(TagKind::Hn { level });

    tag.attrs.set(Attr::Struct(StructAttr::HeadingLevel(level)));
    if let Some(title) = title {
      tag.attrs.set(Attr::Struct(StructAttr::Title(title)));
    }
    tag
  }

  pub fn list(numbering: ListNumbering) -> Self {
    let mut tag = Self::plain(TagKind::L);

    tag.attrs.set(Attr::List(ListAttr::Numbering(numbering)));
    tag
  }

  pub fn table_header(
    scope: TableHeaderScope,
    row_span: Option<NonZeroU32>,
    col_span: Option<NonZeroU32>,
  ) -> Self {
    let mut tag = Self::plain(TagKind::TH);

    tag.attrs.set(Attr::Table(TableAttr::HeaderScope(scope)));
    if let Some(row_span) = row_span {
      tag.attrs.set(Attr::Table(TableAttr::RowSpan(row_span)));
    }
    if let Some(col_span) = col_span {
      tag.attrs.set(Attr::Table(TableAttr::ColSpan(col_span)));
    }
    tag
  }

  pub fn table_data(row_span: Option<NonZeroU32>, col_span: Option<NonZeroU32>) -> Self {
    let mut tag = Self::plain(TagKind::TD);

    if let Some(row_span) = row_span {
      tag.attrs.set(Attr::Table(TableAttr::RowSpan(row_span)));
    }
    if let Some(col_span) = col_span {
      tag.attrs.set(Attr::Table(TableAttr::ColSpan(col_span)));
    }
    tag
  }

  pub fn figure(alt_text: Option<String>) -> Self {
    let mut tag = Self::plain(TagKind::Figure);

    if let Some(alt_text) = alt_text {
      tag.attrs.set(Attr::Struct(StructAttr::AltText(alt_text)));
    }
    tag
  }

  pub fn set_id(&mut self, id: Option<TagId>) {
    match id {
      Some(id) => self.attrs.set(Attr::Struct(StructAttr::Id(id))),
      None => self.attrs.remove(StructAttr::ID),
    }
  }

  pub(crate) fn id(&self) -> Option<&TagId> {
    match self.attrs.get(StructAttr::ID) {
      Some(Attr::Struct(StructAttr::Id(id))) => Some(id),
      _ => None,
    }
  }

  pub(crate) fn headers(&self) -> Option<&[TagId]> {
    match self.attrs.get(TableAttr::CELL_HEADERS) {
      Some(Attr::Table(TableAttr::CellHeaders(headers))) => Some(headers),
      _ => None,
    }
  }

  pub(crate) fn title(&self) -> Option<&str> {
    match self.attrs.get(StructAttr::TITLE) {
      Some(Attr::Struct(StructAttr::Title(title))) => Some(title),
      _ => None,
    }
  }

  pub(crate) fn alt_text(&self) -> Option<&str> {
    match self.attrs.get(StructAttr::ALT_TEXT) {
      Some(Attr::Struct(StructAttr::AltText(alt_text))) => Some(alt_text),
      _ => None,
    }
  }

  #[cfg(test)]
  pub(crate) fn with_attribute(mut self, attr: Attr) -> Self {
    self.attrs.set(attr);
    self
  }

  pub(crate) fn is_list_item(&self) -> bool {
    matches!(self.kind, TagKind::LI)
  }

  pub(crate) fn is_link(&self) -> bool {
    matches!(self.kind, TagKind::Link)
  }

  pub(crate) fn is_list(&self) -> bool {
    matches!(self.kind, TagKind::L)
  }

  pub(crate) fn is_figure(&self) -> bool {
    matches!(self.kind, TagKind::Figure)
  }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TagKind {
  Part,
  Article,
  Section,
  Div,
  BlockQuote,
  Caption,
  TOC,
  TOCI,
  Index,
  P,
  Hn { level: NonZeroU16 },
  L,
  LI,
  Lbl,
  LBody,
  Table,
  TR,
  TH,
  TD,
  THead,
  TBody,
  TFoot,
  Span,
  InlineQuote,
  Note,
  Reference,
  BibEntry,
  Code,
  Link,
  Annot,
  Figure,
  Formula,
  Form,
  NonStruct,
  Datetime,
  Terms,
  Title,
  Strong,
  Em,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct TagId(pub(crate) SmallVec<[u8; 16]>);

impl<I: IntoIterator<Item = u8>> From<I> for TagId {
  fn from(value: I) -> Self {
    Self(std::iter::once(b'U').chain(value).collect())
  }
}

impl TagId {
  pub fn as_bytes(&self) -> &[u8] {
    self.0.as_slice()
  }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum ListNumbering {
  None,
  Disc,
  Circle,
  Square,
  Decimal,
  LowerRoman,
  UpperRoman,
  LowerAlpha,
  UpperAlpha,
}

impl ListNumbering {
  pub(crate) fn to_pdf(self) -> PdfListNumbering {
    match self {
      Self::None => PdfListNumbering::None,
      Self::Disc => PdfListNumbering::Disc,
      Self::Circle => PdfListNumbering::Circle,
      Self::Square => PdfListNumbering::Square,
      Self::Decimal => PdfListNumbering::Decimal,
      Self::LowerRoman => PdfListNumbering::LowerRoman,
      Self::UpperRoman => PdfListNumbering::UpperRoman,
      Self::LowerAlpha => PdfListNumbering::LowerAlpha,
      Self::UpperAlpha => PdfListNumbering::UpperAlpha,
    }
  }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum TableHeaderScope {
  Row,
  Column,
  Both,
}

impl TableHeaderScope {
  pub(crate) fn to_pdf(self) -> PdfTableHeaderScope {
    match self {
      Self::Row => PdfTableHeaderScope::Row,
      Self::Column => PdfTableHeaderScope::Column,
      Self::Both => PdfTableHeaderScope::Both,
    }
  }
}

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
    for (index, item) in self.items.iter().enumerate() {
      if item.ordinal() == attr.ordinal() {
        self.items[index] = attr;
        return;
      }
      if item.ordinal() > attr.ordinal() {
        self.items.insert(index, attr);
        return;
      }
    }
    self.items.push(attr);
  }

  pub(crate) fn remove(&mut self, ordinal: usize) {
    if let Some(index) = self.items.iter().position(|item| item.ordinal() == ordinal) {
      self.items.remove(index);
    }
  }

  pub(crate) fn get(&self, ordinal: usize) -> Option<&A> {
    self.items.iter().find(|item| item.ordinal() == ordinal)
  }
}

pub(crate) trait Ordinal {
  fn ordinal(&self) -> usize;
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
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Attr {
  Struct(StructAttr),
  List(ListAttr),
  Table(TableAttr),
  Layout(LayoutAttr),
}

impl Ordinal for Attr {
  fn ordinal(&self) -> usize {
    match self {
      Self::Struct(a) => a.ordinal(),
      Self::List(a) => a.ordinal(),
      Self::Table(a) => a.ordinal(),
      Self::Layout(a) => a.ordinal(),
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum StructAttr {
  /// The tag id.
  Id(TagId),
  /// The language of this tag.
  Lang(String),
  /// The optional alternate text that describes the text (for example, if the text
  /// consists of a star symbol, the alt text should describe that in natural language).
  AltText(String),
  /// The expanded form of an abbreviation.
  /// Only applicable if the content of the tag is an abbreviation.
  Expanded(String),
  /// The actual text represented by the content of this tag, i.e. if it contained
  /// some curves that artistically represent some word. This should be the exact
  /// replacement text of the word.
  ActualText(String),
  /// The title, characterizing a specific tag such as `"Chapter 1"`.
  Title(String),
  /// The heading level
  HeadingLevel(NonZeroU16),
}

impl StructAttr {
  pub(crate) const ID: usize = 0;
  pub(crate) const LANG: usize = 1;
  pub(crate) const ALT_TEXT: usize = 2;
  pub(crate) const EXPANDED: usize = 3;
  pub(crate) const ACTUAL_TEXT: usize = 4;
  pub(crate) const TITLE: usize = 5;
  pub(crate) const HEADING_LEVEL: usize = 6;
}

impl Ordinal for StructAttr {
  fn ordinal(&self) -> usize {
    match self {
      Self::Id(_) => Self::ID,
      Self::Lang(_) => Self::LANG,
      Self::AltText(_) => Self::ALT_TEXT,
      Self::Expanded(_) => Self::EXPANDED,
      Self::ActualText(_) => Self::ACTUAL_TEXT,
      Self::Title(_) => Self::TITLE,
      Self::HeadingLevel(_) => Self::HEADING_LEVEL,
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ListAttr {
  /// The list numbering.
  Numbering(ListNumbering),
}

impl ListAttr {
  pub(crate) const NUMBERING: usize = 7;
}

impl Ordinal for ListAttr {
  fn ordinal(&self) -> usize {
    match self {
      Self::Numbering(_) => Self::NUMBERING,
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TableAttr {
  /// The table summary.
  Summary(String),
  /// The table header scope.
  HeaderScope(TableHeaderScope),
  /// The list of headers associated with a table cell.
  /// Table data cells (`TD`) may specify a list of table headers (`TH`),
  /// which can also specify a list of parent header cells (`TH`), and so on.
  /// To determine the list of associated headers this list is recursively
  /// evaluated.
  ///
  /// This allows specifying header hierarchies inside tables.
  CellHeaders(SmallVec<[TagId; 1]>),
  /// The row span of this table cell.
  RowSpan(NonZeroU32),
  /// The column span of this table cell.
  ColSpan(NonZeroU32),
}

impl TableAttr {
  pub(crate) const SUMMARY: usize = 8;
  pub(crate) const HEADER_SCOPE: usize = 9;
  pub(crate) const CELL_HEADERS: usize = 10;
  pub(crate) const ROW_SPAN: usize = 11;
  pub(crate) const COL_SPAN: usize = 12;
}

impl Ordinal for TableAttr {
  fn ordinal(&self) -> usize {
    match self {
      Self::Summary(_) => Self::SUMMARY,
      Self::HeaderScope(_) => Self::HEADER_SCOPE,
      Self::CellHeaders(_) => Self::CELL_HEADERS,
      Self::RowSpan(_) => Self::ROW_SPAN,
      Self::ColSpan(_) => Self::COL_SPAN,
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum LayoutAttr {
  /// The placement.
  Placement(Placement),
  /// The writing mode.
  WritingMode(WritingMode),
  /// The bounding box of a tag that encloses its visible content.
  /// If the content spans multiple pages, this should be omitted.
  BBox(BBox),
  /// The width.
  Width(f32),
  /// The height.
  Height(f32),
  /// The background color.
  BackgroundColor(NaiveRgbColor),
  /// The border color.
  BorderColor(Sides<NaiveRgbColor>),
  /// The way the border is drawn.
  BorderStyle(Sides<BorderStyle>),
  /// The border width.
  BorderThickness(Sides<f32>),
  /// The padding inside of an element.
  Padding(Sides<f32>),
  /// The color of text, borders, and text decorations.
  Color(NaiveRgbColor),
  /// The spacing before the block-level element.
  SpaceBefore(f32),
  /// The spacing after the block-level element.
  SpaceAfter(f32),
  /// The spacing between the start inline edge of the element and the parent.
  StartIndent(f32),
  /// The spacing between the end inline edge of the element and the parent.
  EndIndent(f32),
  /// The amount the first line of text in a block-level element is indented. Only
  /// applicable to paragraph-like elements with non-block-level elements.
  TextIndent(f32),
  /// The text alignment.
  TextAlign(TextAlign),
  /// The alignment of block-level elements inside of this block-level element.
  BlockAlign(BlockAlign),
  /// The alignment of inline-level elements inside of this block-level element.
  InlineAlign(InlineAlign),
  /// The border style of table cells, overriding `BorderStyle`.
  TableBorderStyle(Sides<BorderStyle>),
  /// The padding inside of table cells, overriding `Padding`.
  TablePadding(Sides<f32>),
  /// The distance by which the baseline shall be shifted from the default position.
  BaselineShift(f32),
  /// The height of each line in an element on the block axis.
  LineHeight(LineHeight),
  /// The color of the text decoration, overriding the fill color.
  TextDecorationColor(NaiveRgbColor),
  /// The width of the text decoration line.
  TextDecorationThickness(f32),
  /// The kind of text decoration.
  TextDecorationType(TextDecorationType),
  /// How the glyphs are rotated in a vertical writing mode.
  GlyphOrientationVertical(GlyphOrientationVertical),
  /// The number of columns in the grouping element.
  ColumnCount(NonZeroU32),
  /// The width of the gaps between columns in the grouping element.
  ColumnGap(ColumnDimensions),
  /// The width of the columns in the grouping element.
  ColumnWidths(ColumnDimensions),
}

impl LayoutAttr {
  pub(crate) const PLACEMENT: usize = 13;
  pub(crate) const WRITING_MODE: usize = 14;
  pub(crate) const B_BOX: usize = 15;
  pub(crate) const WIDTH: usize = 16;
  pub(crate) const HEIGHT: usize = 17;
  pub(crate) const BACKGROUND_COLOR: usize = 18;
  pub(crate) const BORDER_COLOR: usize = 19;
  pub(crate) const BORDER_STYLE: usize = 20;
  pub(crate) const BORDER_THICKNESS: usize = 21;
  pub(crate) const PADDING: usize = 22;
  pub(crate) const COLOR: usize = 23;
  pub(crate) const SPACE_BEFORE: usize = 24;
  pub(crate) const SPACE_AFTER: usize = 25;
  pub(crate) const START_INDENT: usize = 26;
  pub(crate) const END_INDENT: usize = 27;
  pub(crate) const TEXT_INDENT: usize = 28;
  pub(crate) const TEXT_ALIGN: usize = 29;
  pub(crate) const BLOCK_ALIGN: usize = 30;
  pub(crate) const INLINE_ALIGN: usize = 31;
  pub(crate) const TABLE_BORDER_STYLE: usize = 32;
  pub(crate) const TABLE_PADDING: usize = 33;
  pub(crate) const BASELINE_SHIFT: usize = 34;
  pub(crate) const LINE_HEIGHT: usize = 35;
  pub(crate) const TEXT_DECORATION_COLOR: usize = 36;
  pub(crate) const TEXT_DECORATION_THICKNESS: usize = 37;
  pub(crate) const TEXT_DECORATION_TYPE: usize = 38;
  pub(crate) const GLYPH_ORIENTATION_VERTICAL: usize = 39;
  pub(crate) const COLUMN_COUNT: usize = 40;
  pub(crate) const COLUMN_GAP: usize = 41;
  pub(crate) const COLUMN_WIDTHS: usize = 42;
}

impl Ordinal for LayoutAttr {
  fn ordinal(&self) -> usize {
    match self {
      Self::Placement(_) => Self::PLACEMENT,
      Self::WritingMode(_) => Self::WRITING_MODE,
      Self::BBox(_) => Self::B_BOX,
      Self::Width(_) => Self::WIDTH,
      Self::Height(_) => Self::HEIGHT,
      Self::BackgroundColor(_) => Self::BACKGROUND_COLOR,
      Self::BorderColor(_) => Self::BORDER_COLOR,
      Self::BorderStyle(_) => Self::BORDER_STYLE,
      Self::BorderThickness(_) => Self::BORDER_THICKNESS,
      Self::Padding(_) => Self::PADDING,
      Self::Color(_) => Self::COLOR,
      Self::SpaceBefore(_) => Self::SPACE_BEFORE,
      Self::SpaceAfter(_) => Self::SPACE_AFTER,
      Self::StartIndent(_) => Self::START_INDENT,
      Self::EndIndent(_) => Self::END_INDENT,
      Self::TextIndent(_) => Self::TEXT_INDENT,
      Self::TextAlign(_) => Self::TEXT_ALIGN,
      Self::BlockAlign(_) => Self::BLOCK_ALIGN,
      Self::InlineAlign(_) => Self::INLINE_ALIGN,
      Self::TableBorderStyle(_) => Self::TABLE_BORDER_STYLE,
      Self::TablePadding(_) => Self::TABLE_PADDING,
      Self::BaselineShift(_) => Self::BASELINE_SHIFT,
      Self::LineHeight(_) => Self::LINE_HEIGHT,
      Self::TextDecorationColor(_) => Self::TEXT_DECORATION_COLOR,
      Self::TextDecorationThickness(_) => Self::TEXT_DECORATION_THICKNESS,
      Self::TextDecorationType(_) => Self::TEXT_DECORATION_TYPE,
      Self::GlyphOrientationVertical(_) => Self::GLYPH_ORIENTATION_VERTICAL,
      Self::ColumnCount(_) => Self::COLUMN_COUNT,
      Self::ColumnGap(_) => Self::COLUMN_GAP,
      Self::ColumnWidths(_) => Self::COLUMN_WIDTHS,
    }
  }
}
