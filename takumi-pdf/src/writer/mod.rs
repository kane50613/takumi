// Vendored from krilla 0.8.2 (https://github.com/LaurenzV/krilla, MIT OR
// Apache-2.0), stripped of the `pdf`, `simple-text`, `comemo` and `rayon`
// features and of everything takumi-pdf does not call.

//! The PDF writer under takumi-pdf: documents, pages, a drawing surface, fonts, images, and
//! the tag tree, serialized through pdf-writer. Only what takumi draws is kept.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

mod chunk_container;
mod graphics;
mod interactive;
mod interchange;
mod object_stream;
mod resource;
mod serialize;
mod util;

pub(crate) mod content;
pub(crate) mod data;

pub mod configure;
pub mod document;
pub mod error;
pub mod geom;
pub mod num;
pub mod page;
pub mod stream;
pub mod surface;
pub mod text;

pub use data::*;
pub use document::*;
pub use graphics::*;
pub use interactive::*;
pub use interchange::*;
pub use serialize::SerializeSettings;
