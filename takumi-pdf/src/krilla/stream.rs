//! Drawing to a non-page context.
//!
//! In 90% of the cases, you will not have to use streams. In most cases,
//! all you need to do when using this crate is to first construct a document,
//! and then add new pages to the document and use the [`Page::surface`] method to get
//! access to the drawing context. However, there are cases when you don't want to
//! draw on the main page surface, but instead you want to create a "sub-surface"
//! where you can draw independently of the main page contents. This is what streams
//! are there for. Currently, there are only two situations where you need to do that:
//!
//! - When using masks and defining the contents of the mask.
//! - When using a pattern fill or stroke and defining the contents of the pattern.
//!
//! If you want to do any of the two above, you need to call the [`Surface::stream_builder`] method
//! of the current surface. The stream builder represents a kind of sub-context that is
//! independent of the main surface you are working with. Once you have a stream builder, you
//! can once again invoke the [`StreamBuilder::surface`] method, and use this new surface to define the contents
//! of your mask/pattern. In the end, you can call [`StreamBuilder::finish`] which will return a [`Stream`] object.
//! This [`Stream`] object contains the encoded instructions of the mask/pattern, which you can
//! then use to create new [`Pattern`]/[`Mask`] objects.
//!
//! [`Page::surface`]: crate::krilla::page::Page::surface
//! [`Surface::stream_builder`]: crate::krilla::surface::Surface::stream_builder
//! [`Pattern`]: crate::krilla::graphics::paint::Pattern
//! [`Mask`]: crate::krilla::graphics::mask::Mask

use std::borrow::Cow;
use std::ops::DerefMut;

use flate2::Compression;
use flate2::write::ZlibEncoder;
use pdf_writer::{Array, Dict, Name};
#[cfg(feature = "raster-images")]
use pdf_writer::{Finish, Null};

use crate::krilla::SerializeSettings;
use crate::krilla::chunk_container::ChunkContainer;
use crate::krilla::configure::ValidationError;
use crate::krilla::content::ContentBuilder;
use crate::krilla::geom::{Rect, Transform};
use crate::krilla::resource::{ResourceDictionary, ResourceDictionaryBuilder};
use crate::krilla::serialize::SerializeContext;
use crate::krilla::surface::Surface;

/// A stream.
///
/// See the module description for an explanation of its purpose.
// The only reason we implement clone for this type is that in some cases,
// we might need to clone a pattern (including its stream)
#[derive(Debug, Hash, Eq, PartialEq, Clone)]
pub struct Stream {
  pub(crate) content: Vec<u8>,
  pub(crate) bbox: Rect,
  // Important: Each object that uses a stream must ensure to pass on the validation
  // errors to the `SerializeContext` at some point. Currently, only `Mask`,
  // `TilingPattern`, `InternalPage` and `XObject` require that.
  pub(crate) validation_errors: Vec<ValidationError>,
  pub(crate) resource_dictionary: ResourceDictionary,
  pub(crate) uses_mask: bool,
}

impl Stream {
  pub(crate) fn new(
    content: Vec<u8>,
    bbox: Rect,
    validation_errors: Vec<ValidationError>,
    resource_dictionary: ResourceDictionary,
    uses_mask: bool,
  ) -> Self {
    Self {
      content,
      bbox,
      validation_errors,
      resource_dictionary,
      uses_mask,
    }
  }

  pub(crate) fn is_empty(&self) -> bool {
    self.content.is_empty()
  }

  pub(crate) fn empty() -> Self {
    Self {
      content: vec![],
      bbox: Rect::from_xywh(0.0, 0.0, 0.0, 0.0).unwrap(),
      validation_errors: vec![],
      resource_dictionary: ResourceDictionaryBuilder::new().finish(),
      uses_mask: false,
    }
  }
}

/// A builder to create streams.
pub struct StreamBuilder<'a> {
  sc: &'a mut SerializeContext,
  chunk_container: &'a mut ChunkContainer,
  stream: Stream,
}

impl<'a> StreamBuilder<'a> {
  pub(crate) fn new(sc: &'a mut SerializeContext, chunk_container: &'a mut ChunkContainer) -> Self {
    Self {
      sc,
      chunk_container,
      stream: Stream::empty(),
    }
  }

  /// Get the surface of the stream builder.
  pub fn surface(&mut self) -> Surface<'_> {
    // Stream builders cannot have any tags since we always pass a dummy
    // identifier. Only main page content streams can have one.
    let finish_fn = Box::new(|stream, _| {
      self.stream = stream;
    });

    Surface::new(
      self.sc,
      self.chunk_container,
      ContentBuilder::new(Transform::identity(), true, self.sc),
      None,
      finish_fn,
    )
  }

  /// Turn the stream builder into a stream.
  pub fn finish(self) -> Stream {
    self.stream
  }
}

/// A PDF stream filter.
#[derive(Debug, Copy, Clone)]
pub(crate) enum StreamFilter {
  Flate,
  FlateMemoized,
  AsciiHex,
  #[cfg(feature = "raster-images")]
  Dct,
}

impl StreamFilter {
  pub(crate) fn to_name(self) -> Name<'static> {
    match self {
      Self::AsciiHex => Name(b"ASCIIHexDecode"),
      Self::Flate => Name(b"FlateDecode"),
      Self::FlateMemoized => Name(b"FlateDecode"),
      #[cfg(feature = "raster-images")]
      Self::Dct => Name(b"DCTDecode"),
    }
  }

  pub(crate) fn is_binary(&self) -> bool {
    match self {
      StreamFilter::Flate => true,
      StreamFilter::FlateMemoized => true,
      StreamFilter::AsciiHex => false,
      #[cfg(feature = "raster-images")]
      StreamFilter::Dct => true,
    }
  }
}

impl StreamFilter {
  #[track_caller]
  pub(crate) fn apply(&self, content: &[u8]) -> Vec<u8> {
    match self {
      StreamFilter::Flate => deflate_encode(content),
      StreamFilter::FlateMemoized => deflate_encode_memoized(content),
      StreamFilter::AsciiHex => hex_encode(content),
      // Note: We don't actually encode manually with DCT, because
      // this is only used for JPEG images which are already encoded,
      // so this shouldn't be called at all.
      #[cfg(feature = "raster-images")]
      StreamFilter::Dct => panic!("can't apply dct decode"),
    }
  }
}

/// A stream filter together with the `/DecodeParms` entry it needs.
#[derive(Debug, Copy, Clone)]
struct Filter {
  kind: StreamFilter,
  #[cfg(feature = "raster-images")]
  decode_params: Option<DecodeParams>,
}

impl Filter {
  fn new(kind: StreamFilter) -> Self {
    Self {
      kind,
      #[cfg(feature = "raster-images")]
      decode_params: None,
    }
  }

  #[cfg(feature = "raster-images")]
  fn with_decode_params(mut self, decode_params: DecodeParams) -> Self {
    self.decode_params = Some(decode_params);
    self
  }
}

#[cfg(feature = "raster-images")]
#[derive(Debug, Copy, Clone)]
enum DecodeParams {
  PngPredictor {
    colors: i32,
    bits_per_component: i32,
    columns: i32,
  },
}

#[cfg(feature = "raster-images")]
impl DecodeParams {
  fn write(self, mut dict: Dict<'_>) {
    match self {
      Self::PngPredictor {
        colors,
        bits_per_component,
        columns,
      } => {
        dict.pair(Name(b"Predictor"), 15);
        dict.pair(Name(b"Colors"), colors);
        dict.pair(Name(b"BitsPerComponent"), bits_per_component);
        dict.pair(Name(b"Columns"), columns);
      }
    }

    dict.finish();
  }
}

/// Allows us to keep track of the filters that a stream has and
/// apply them in an orderly fashion.
#[derive(Debug, Clone)]
enum StreamFilters {
  None,
  Single(Filter),
  Multiple(Vec<Filter>),
}

impl StreamFilters {
  fn add(&mut self, filter: Filter) {
    match self {
      StreamFilters::None => *self = StreamFilters::Single(filter),
      StreamFilters::Single(cur) => *self = StreamFilters::Multiple(vec![*cur, filter]),
      StreamFilters::Multiple(cur) => cur.push(filter),
    }
  }

  fn is_binary(&self) -> bool {
    match self {
      StreamFilters::None => false,
      StreamFilters::Single(filter) => filter.kind.is_binary(),
      StreamFilters::Multiple(filters) => filters.last().unwrap().kind.is_binary(),
    }
  }
}

pub(crate) struct FilterStreamBuilder<'a> {
  content: Cow<'a, [u8]>,
  filters: StreamFilters,
  is_binary: bool,
}

impl<'a> FilterStreamBuilder<'a> {
  fn empty(content: &'a [u8]) -> Self {
    Self {
      content: Cow::Borrowed(content),
      filters: StreamFilters::None,
      is_binary: false,
    }
  }

  fn binary(content: &'a [u8]) -> Self {
    Self {
      is_binary: true,
      ..Self::empty(content)
    }
  }

  pub(crate) fn new_from_content_stream(
    content: &'a [u8],
    serialize_settings: &SerializeSettings,
  ) -> Self {
    let mut filter_stream = Self::empty(content);

    if serialize_settings.compress_content_streams {
      filter_stream.add_filter(StreamFilter::FlateMemoized);
    }

    filter_stream
  }

  pub(crate) fn new_from_deflated(content: &'a [u8]) -> Self {
    let mut filter_stream = Self::empty(content);
    filter_stream.add_unapplied_filter(StreamFilter::Flate);

    filter_stream
  }

  pub(crate) fn new_from_binary_data(content: &'a [u8]) -> Self {
    let mut filter_stream = Self::binary(content);
    filter_stream.add_filter(StreamFilter::Flate);

    filter_stream
  }

  pub(crate) fn new_auto_compressed(content: &'a [u8]) -> Self {
    let filter_stream = Self::new_from_binary_data(content);

    const MAX_COMPRESSED_SIZE: usize = 75;
    if content.is_empty() || 100 * filter_stream.content.len() / content.len() > MAX_COMPRESSED_SIZE
    {
      return Self::binary(content);
    }

    filter_stream
  }

  pub(crate) fn new_from_uncompressed(content: &'a [u8]) -> Self {
    Self::binary(content)
  }

  #[cfg(feature = "raster-images")]
  pub(crate) fn new_from_jpeg_data(content: &'a [u8]) -> Self {
    let mut filter_stream = Self::empty(content);
    // JPEG data already is DCT encoded.
    filter_stream.add_unapplied_filter(StreamFilter::Dct);

    filter_stream
  }

  /// A PNG IDAT stream, already deflated with the PNG row predictors applied.
  #[cfg(feature = "raster-images")]
  pub(crate) fn new_from_png_data(
    content: &'a [u8],
    colors: i32,
    bits_per_component: i32,
    columns: i32,
  ) -> Self {
    let mut filter_stream = Self::empty(content);
    filter_stream
      .filters
      .add(
        Filter::new(StreamFilter::Flate).with_decode_params(DecodeParams::PngPredictor {
          colors,
          bits_per_component,
          columns,
        }),
      );

    filter_stream
  }

  pub(crate) fn finish(mut self, serialize_settings: &SerializeSettings) -> FilterStream<'a> {
    if serialize_settings.ascii_compatible
      && !self.content.is_empty()
      && (self.filters.is_binary()
        || (self.is_binary && self.content.iter().any(|byte| !byte.is_ascii())))
    {
      self.add_filter(StreamFilter::AsciiHex);
    }

    FilterStream {
      content: self.content,
      filters: self.filters,
    }
  }

  fn add_filter(&mut self, filter: StreamFilter) {
    self.content = Cow::Owned(filter.apply(&self.content));
    self.filters.add(Filter::new(filter));
  }

  fn add_unapplied_filter(&mut self, filter: StreamFilter) {
    self.filters.add(Filter::new(filter));
  }
}

pub(crate) struct FilterStream<'a> {
  content: Cow<'a, [u8]>,
  filters: StreamFilters,
}

impl FilterStream<'_> {
  pub(crate) fn encoded_data(&self) -> &[u8] {
    &self.content
  }

  pub(crate) fn write_filters<'b, T>(&self, mut dict: T)
  where
    T: DerefMut<Target = Dict<'b>>,
  {
    match &self.filters {
      StreamFilters::None => {}
      StreamFilters::Single(filter) => {
        dict
          .deref_mut()
          .pair(Name(b"Filter"), filter.kind.to_name());

        #[cfg(feature = "raster-images")]
        if let Some(params) = filter.decode_params {
          params.write(dict.deref_mut().insert(Name(b"DecodeParms")).dict());
        }
      }
      StreamFilters::Multiple(filters) => {
        dict
          .deref_mut()
          .insert(Name(b"Filter"))
          .start::<Array>()
          .items(filters.iter().rev().map(|filter| filter.kind.to_name()));

        #[cfg(feature = "raster-images")]
        if filters.iter().any(|filter| filter.decode_params.is_some()) {
          let mut decode_params = dict.deref_mut().insert(Name(b"DecodeParms")).array();

          for filter in filters.iter().rev() {
            match filter.decode_params {
              Some(params) => params.write(decode_params.push().dict()),
              None => decode_params.push().primitive(Null),
            }
          }

          decode_params.finish();
        }
      }
    }
  }
}

fn deflate_encode_memoized(data: &[u8]) -> Vec<u8> {
  deflate_encode(data)
}

pub(crate) fn deflate_encode(data: &[u8]) -> Vec<u8> {
  use std::io::Write;

  const COMPRESSION_LEVEL: u8 = 6;
  let mut e = ZlibEncoder::new(Vec::new(), Compression::new(COMPRESSION_LEVEL as u32));
  e.write_all(data).unwrap();
  e.finish().unwrap()
}

fn hex_encode(data: &[u8]) -> Vec<u8> {
  use std::fmt::Write;

  const COLUMNS: usize = 35;
  let len = data.len();
  let capacity = 2 * len + len / COLUMNS;
  let mut output = String::with_capacity(capacity);
  for (index, byte) in data.iter().enumerate() {
    write!(output, "{byte:02X}").unwrap();
    if index % COLUMNS == COLUMNS - 1 {
      output.push('\n');
    }
  }
  output.into_bytes()
}
