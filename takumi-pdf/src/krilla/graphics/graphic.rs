//! Drawing graphics. This allows you to reuse the same graphical content
//! multiple times in the PDF without incurring any overhead in terms of file size.

use crate::krilla::graphics::xobject::XObject;
use crate::krilla::stream::Stream;

/// A cacheable graphic. You can use this for large graphics objects which you
/// want to reuse in multiple locations in the same document. Embedding the same
/// graphic multiple times will ensure that it is only actually written once in the
/// PDF document, which might lead to better file sizes.
///
/// IMPORTANT: Note that you must only use a graphic in the document that you created it with!
/// If you use it in a different document, you will end up with an invalid PDF file.
#[derive(PartialEq, Eq, Debug, Hash, Clone)]
pub struct Graphic {
  /// The stream of the graphic.
  pub(crate) x_object: XObject,
}

impl Graphic {
  /// Create a new graphic.
  /// `stream` contains the content description of the graphic.
  /// `isolated` indicates whether the contents of the graphic should be isolated
  /// from wherever the graphic is invoked.
  pub fn new(stream: Stream, isolated: bool) -> Self {
    Self {
      x_object: XObject::new(stream, isolated, false, None),
    }
  }
}
