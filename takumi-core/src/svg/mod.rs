//! SVG, parsed and styled the way Blink's `core/svg` does.
//!
//! [`Document`] holds the element tree with every element's [`SvgStyle`] computed. `layout`
//! turns it into a render tree (`tree`), and `ops` flattens that into the drawing ops every
//! backend draws. `tree`, `text`, and `filter` derive from resvg 0.48.1 (Apache-2.0 OR MIT),
//! with text shaped by takumi's fonts in place of `fontdb` and `rustybuzz`. `layout`
//! turns it into a render tree (`tree`), and `ops` flattens that into the drawing ops every
//! backend draws. `tree`, `text`, and `filter` derive from resvg 0.48.1 (Apache-2.0 OR MIT),
//! with text shaped by takumi's fonts in place of `fontdb` and `rustybuzz`.

mod cascade;
mod document;
#[cfg(feature = "svg-raster")]
pub(crate) mod drawing;
#[cfg(feature = "svg-raster")]
#[allow(
  clippy::unwrap_used,
  clippy::expect_used,
  clippy::panic,
  clippy::all,
  missing_docs,
  dead_code,
  deprecated
)]
pub(crate) mod filter;
#[cfg(feature = "svg")]
pub(crate) mod layout;
mod length;
mod name;
#[cfg(feature = "svg")]
pub(crate) mod ops;
#[cfg(feature = "svg")]
mod option_log;
#[cfg(feature = "svg")]
mod options;
mod path;
mod scan;
mod style;
pub(crate) mod style_sheet;
#[cfg(feature = "svg")]
#[allow(
  clippy::unwrap_used,
  clippy::expect_used,
  clippy::panic,
  clippy::all,
  missing_docs,
  dead_code,
  deprecated
)]
mod text;
mod transform;
#[cfg(feature = "svg")]
#[allow(
  clippy::unwrap_used,
  clippy::expect_used,
  clippy::panic,
  clippy::all,
  missing_docs,
  dead_code,
  deprecated
)]
pub(crate) mod tree;
mod view_box;

pub use document::{Document, HrefChain, Node, NodeId, ParseError, ParseOptions};
pub use length::{Length, LengthContext, LengthMode, LengthUnit};
pub use name::{Attr, Property, Tag};
pub use path::{PathSegment, parse_path_data, parse_path_data_strict};
pub use scan::{
  Scanner, parse_angle, parse_length, parse_length_list, parse_number, parse_number_list,
};
pub use style::*;
pub use transform::{TransformOrigin, parse_transform_list, parse_transform_origin};
pub use view_box::{Align, PreserveAspectRatio, ViewBox};
