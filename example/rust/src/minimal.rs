// Import necessary modules for file operations and takumi library components
use std::fs::File;

use takumi::{
  GlobalContext,
  layout::{
    Viewport,
    node::{ContainerNode, NodeKind, TextNode},
    style::{CssOption, StyleBuilder},
  },
  rendering::{ImageOutputFormat, RenderOptionsBuilder, render, write_image},
};

/// Generates a "Hello, {name}!" image with specified dimensions and styling
///
/// This function creates a simple image with text using the takumi library.
/// The image is rendered and saved as a WebP file named "output.webp".
pub fn say_hello_to(name: &str) {
  // Create a new context for rendering with default settings,
  // Only one instance should be created for the entire application,
  // you can wrap it in a Arc<Context> to share it across threads if needed.
  let context = GlobalContext::default();

  // Font loading can be done here if you have custom fonts to use,
  // by default, takumi WON'T load any system fonts.
  //
  // Example:
  // context.font_context.load_and_store()

  // Create a text node with custom styling
  // Font size is set to 48.0 and other styles use default values
  let text = TextNode {
    tw: None,
    style: Some(
      StyleBuilder::default()
        .font_size(CssOption::some(48.0.into()))
        .build()
        .unwrap(),
    ),
    text: format!("Hello, {name}!"),
  };

  // Create a root container node that will hold the text
  // Set dimensions to 1200x630 pixels (common size for social media images)
  let root: ContainerNode<NodeKind> = ContainerNode {
    tw: None,
    style: Default::default(),
    children: Some(vec![text.into()]),
  };

  // Create render options
  let options = RenderOptionsBuilder::<NodeKind>::default()
    .viewport(Viewport::new(1200, 630))
    .node(root.into())
    .global(&context)
    .build()
    .unwrap();

  // Create an image from the render options
  let image = render(options).unwrap();

  // Create a new file to save the rendered image
  let mut file = File::create("output.webp").unwrap();

  // Write the image to the file in WebP format
  write_image(&image, &mut file, ImageOutputFormat::WebP, None).unwrap();
}
