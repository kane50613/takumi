//! SVG, parsed and styled the way Blink's `core/svg` does.
//!
//! [`Document`] holds the element tree with every element's [`SvgStyle`] computed.

mod cascade;
mod document;
mod length;
mod name;
mod scan;
mod style;
pub(crate) mod style_sheet;
mod transform;
mod view_box;

pub use document::{Document, Node, NodeId, ParseError, ParseOptions};
pub use length::{Length, LengthContext, LengthMode, LengthUnit};
pub use name::{Attr, Property, Tag};
pub use scan::{Scanner, parse_length, parse_length_list, parse_number, parse_number_list};
pub use style::*;
pub use transform::parse_transform_list;
pub use view_box::{Align, PreserveAspectRatio, ViewBox};
