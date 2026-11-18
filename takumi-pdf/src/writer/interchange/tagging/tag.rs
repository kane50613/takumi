use std::num::{NonZeroU16, NonZeroU32};

use pdf_writer::types::{
  ListNumbering as PdfListNumbering, TableHeaderScope as PdfTableHeaderScope,
};
use smallvec::SmallVec;

use crate::writer::surface::Location;

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
  Article,
  Section,
  BlockQuote,
  Caption,
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
  Code,
  Link,
  Figure,
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
}

impl TableHeaderScope {
  pub(crate) fn to_pdf(self) -> PdfTableHeaderScope {
    match self {
      Self::Row => PdfTableHeaderScope::Row,
      Self::Column => PdfTableHeaderScope::Column,
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
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Attr {
  Struct(StructAttr),
  List(ListAttr),
  Table(TableAttr),
}

impl Ordinal for Attr {
  fn ordinal(&self) -> usize {
    match self {
      Self::Struct(a) => a.ordinal(),
      Self::List(a) => a.ordinal(),
      Self::Table(a) => a.ordinal(),
    }
  }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum StructAttr {
  /// The tag id.
  Id(TagId),
  /// The optional alternate text that describes the text (for example, if the text
  /// consists of a star symbol, the alt text should describe that in natural language).
  AltText(String),
  /// The title, characterizing a specific tag such as `"Chapter 1"`.
  Title(String),
  /// The heading level
  HeadingLevel(NonZeroU16),
}

impl StructAttr {
  pub(crate) const ID: usize = 0;
  pub(crate) const ALT_TEXT: usize = 2;
  pub(crate) const TITLE: usize = 5;
  pub(crate) const HEADING_LEVEL: usize = 6;
}

impl Ordinal for StructAttr {
  fn ordinal(&self) -> usize {
    match self {
      Self::Id(_) => Self::ID,
      Self::AltText(_) => Self::ALT_TEXT,
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
  /// The table header scope.
  HeaderScope(TableHeaderScope),
  /// The row span of this table cell.
  RowSpan(NonZeroU32),
  /// The column span of this table cell.
  ColSpan(NonZeroU32),
}

impl TableAttr {
  pub(crate) const HEADER_SCOPE: usize = 9;
  pub(crate) const ROW_SPAN: usize = 11;
  pub(crate) const COL_SPAN: usize = 12;
}

impl Ordinal for TableAttr {
  fn ordinal(&self) -> usize {
    match self {
      Self::HeaderScope(_) => Self::HEADER_SCOPE,
      Self::RowSpan(_) => Self::ROW_SPAN,
      Self::ColSpan(_) => Self::COL_SPAN,
    }
  }
}
