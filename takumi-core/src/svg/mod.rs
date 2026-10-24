//! SVG, parsed and styled the way Blink's `core/svg` does.
//!
//! [`Document`] holds the element tree with every element's [`SvgStyle`] computed.

mod cascade;
mod document;
#[cfg(feature = "svg")]
pub(crate) mod layout;
mod length;
mod name;
mod path;
mod scan;
mod style;
pub(crate) mod style_sheet;
mod transform;
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
