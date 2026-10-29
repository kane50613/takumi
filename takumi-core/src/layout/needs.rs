//! What a render tree needs from outside before it renders, read after the cascade so stylesheet
//! rules, `display: none`, generated content, and `text-transform` count as they will render.

use std::{collections::BTreeSet, rc::Rc};

use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
  context::RenderContext,
  fonts::{
    font::Font, font_fallback_list::FontFallbackList, font_fallback_priority::FontFallbackPriority,
    symbols_iterator::SymbolsIterator,
  },
  layout::{
    inline::{inline_item::InlineItemType, inline_node::InlineNode},
    node::{ImageSourceInput, NodeKind, is_image_reference, resolve_image},
    tree::RenderNode,
  },
  resources::image::{ImageSource, is_svg_like},
  style::{BackgroundImage, Display, Float, find_keyframes},
};
#[cfg(feature = "svg")]
use crate::{font_style::SizedFontStyle, fonts::font_description::FontDescription};

/// The images a render tree looks up by URL and the pending fonts its text loads.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RenderNeeds {
  /// The URLs of image elements, of computed `background-image` and `mask-image` values, and of
  /// list items' `list-style-image`, except `data:` URIs and inline SVG the render decodes itself.
  pub images: BTreeSet<String>,
  /// The ids of the pending faces (see [`Fonts::register_pending`](crate::Fonts::register_pending))
  /// the render would load: for each of those characters, the first pending face the fallback of
  /// the font it is drawn in meets before a loaded font with its glyph, which Blink's
  /// `FontFallbackIterator` begins loading.
  pub faces: BTreeSet<usize>,
}

impl Extend<RenderNeeds> for RenderNeeds {
  fn extend<I: IntoIterator<Item = RenderNeeds>>(&mut self, reads: I) {
    for read in reads {
      self.images.extend(read.images);
      self.faces.extend(read.faces);
    }
  }
}

impl FromIterator<RenderNeeds> for RenderNeeds {
  fn from_iter<I: IntoIterator<Item = RenderNeeds>>(reads: I) -> Self {
    let mut needs = Self::default();

    needs.extend(reads);

    needs
  }
}

/// The needs a walk collects, with the characters each fallback list has looked up.
#[derive(Default)]
struct NeedsCollector {
  needs: RenderNeeds,
  /// Each fallback list by address, kept alive so no other list takes the address, with the
  /// characters and priorities walked through it.
  walked: FxHashMap<*const FontFallbackList, WalkedCharacters>,
}

/// A fallback list and the characters it has walked, each at a fallback priority.
type WalkedCharacters = (
  Rc<FontFallbackList>,
  FxHashSet<(char, FontFallbackPriority)>,
);

impl RenderNode {
  /// Whether an inline walk passes this box without reading its content, which lays out on its own.
  fn lays_out_apart(&self) -> bool {
    self.is_out_of_flow()
      || self.context.style.base_data.float != Float::None
      || self.is_atomic_inline()
      || self.is_block_in_inline
  }

  /// What this render tree needs before it renders.
  pub fn needs(&self) -> RenderNeeds {
    let mut needs = NeedsCollector::default();

    // The tree builder drops hidden children but keeps a hidden root.
    if self.context.style.base_data.display == Display::None {
      return needs.needs;
    }

    let mut stack = vec![(self, false)];

    // Whether an ancestor's inline walk reads the node's text, which then reads its children's too,
    // except a box it passes because that box lays out on its own.
    while let Some((node, walked)) = stack.pop() {
      needs.add_style(&node.context);
      needs.add_keyframes(&node.context);

      if let Some(NodeKind::Image(image)) = node.node.as_ref().map(|node| &node.kind) {
        if let Some(url) = image.url() {
          needs.needs.images.insert(url.to_owned());
        }

        needs.add_svg_text(svg_image(&image.src, &node.context), &node.context);
      }

      let creates_inline_context = node.should_create_inline_layout();

      // A text laid out on its own, such as a flex item or a float, collects its own content.
      if !walked && (creates_inline_context || node.own_text().is_some()) {
        needs.add_inline_text(node);
      }

      let walks_children = walked || creates_inline_context;

      stack.extend(
        node
          .children
          .iter()
          .flat_map(|children| children.iter())
          .map(|child| (child, walks_children && !child.lays_out_apart())),
      );
    }

    needs.needs
  }
}

impl NeedsCollector {
  fn add_style(&mut self, context: &RenderContext) {
    let style = &context.style;
    let images = [
      style.background_data.background_image.as_ref(),
      style.misc2_data.mask_image.as_ref(),
    ]
    .into_iter()
    .flatten()
    .flat_map(|images| images.iter())
    .chain(
      (style.base_data.display == Display::ListItem)
        .then(|| style.misc_inherited1_data.list_style_image.image())
        .flatten(),
    );

    for image in images {
      if let BackgroundImage::Url(url) = image {
        if is_image_reference(url) {
          self.needs.images.insert(url.to_string());
        }

        self.add_svg_text(svg_url(url, context), context);
      }
    }
  }

  /// Adds the images the `@keyframes` rules the box animates with switch to, which only show at
  /// later frames.
  fn add_keyframes(&mut self, context: &RenderContext) {
    for name in context.style.misc1_data.animation_name.iter().flatten() {
      let Some(rule) = find_keyframes(context.stylesheet(), name, context.sizing.viewport) else {
        continue;
      };

      for url in rule
        .keyframes
        .iter()
        .flat_map(|keyframe| keyframe.declarations.image_urls())
      {
        if is_image_reference(url) {
          self.needs.images.insert(url.to_owned());
        }

        self.add_svg_text(svg_url(url, context), context);
      }
    }
  }

  /// Adds the characters an SVG image's `text` elements draw with the render's fonts, shown in the
  /// box `context` styles, and the pending faces their fonts would load.
  fn add_svg_text(&mut self, image: Option<ImageSource>, context: &RenderContext) {
    #[cfg(feature = "svg")]
    if let Some(ImageSource::Svg(svg)) = image {
      for (context, text) in svg.text_runs(context) {
        let font = context.font(FontDescription::new(&SizedFontStyle::from_style(
          &context.style,
          &context,
        )));

        self.add_text(&font, text);
      }
    }
    #[cfg(not(feature = "svg"))]
    let _ = (image, context);
  }

  /// Adds the text inline layout shapes in the inline formatting context `root` establishes,
  /// collected as layout collects it, `text-transform` applied with the character before each run.
  fn add_inline_text(&mut self, root: &RenderNode) {
    let data = InlineNode::new(root).collect_inlines();

    for item in &data.items {
      if item.item_type != InlineItemType::Text {
        continue;
      }

      let Some(object) = item.layout_object else {
        continue;
      };

      self.add_text(
        &data.fonts.get(object.context()),
        &data.text_content[item.start_offset()..item.end_offset()],
      );
    }
  }

  /// Adds the pending faces `font` would load to draw `text`, each character looked up at the
  /// fallback priority Blink's `SymbolsIterator` gives its run.
  fn add_text(&mut self, font: &Font, text: &str) {
    let list = &font.fallback_list;

    if !list.has_pending_faces() {
      return;
    }

    let (_, walked) = self
      .walked
      .entry(Rc::as_ptr(list))
      .or_insert_with(|| (Rc::clone(list), FxHashSet::default()));
    let mut symbols = SymbolsIterator::new(text);
    let mut start = 0;

    while let Some((end, priority)) = symbols.consume() {
      for character in text[start..end].chars() {
        if !walked.insert((character, priority)) {
          continue;
        }

        if let Some(id) = list.pending_face_for_character(character, priority) {
          self.needs.faces.insert(id);
        }
      }

      start = end;
    }
  }
}

/// The image `src` names when it is one the render can read without fetching: an entry of its
/// `images`, inline markup, or a `data:` URI. A raster `data:` URI stays undecoded, since only an
/// SVG draws text.
fn svg_url(src: &str, context: &RenderContext) -> Option<ImageSource> {
  if is_image_reference(src) {
    return context.images().get(src).cloned();
  }

  let svg_data_uri = src
    .get(..DATA_SVG_PREFIX.len())
    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(DATA_SVG_PREFIX));

  (is_svg_like(src) || svg_data_uri)
    .then(|| resolve_image(src, context).ok())
    .flatten()
}

const DATA_SVG_PREFIX: &str = "data:image/svg+xml";

/// [`svg_url`] for an image element's source, which bytes or a loaded image can also give.
fn svg_image(src: &ImageSourceInput, context: &RenderContext) -> Option<ImageSource> {
  match src {
    ImageSourceInput::Url(url) => svg_url(url, context),
    other => other.resolve(context).ok(),
  }
}

#[cfg(test)]
mod tests {
  use super::RenderNeeds;
  use crate::{
    fonts::shaping::testing::{GEIST_MONO, register},
    layout::{
      inline::testing::{render_tree, render_with_fonts, span},
      node::Node,
    },
    resources::{
      font::{Fonts, PendingFontFace},
      unicode_range::{UnicodeRange, UnicodeRangeSet},
    },
  };

  const POPPINS_REGULAR: &str =
    "../assets/fonts/poppins/poppins-v24-devanagari_latin-regular.woff2";
  const LATIN: UnicodeRange = UnicodeRange::new(0x0, 0xFF);
  const CYRILLIC: UnicodeRange = UnicodeRange::new(0x400, 0x4FF);
  const CJK: UnicodeRange = UnicodeRange::new(0x4E00, 0x9FFF);

  /// A pending face of `family` at `weight` serving `range`.
  fn pending(id: usize, family: &str, weight: f32, range: UnicodeRange) -> PendingFontFace {
    PendingFontFace::new(id, family)
      .weight(weight, weight)
      .unicode_range(UnicodeRangeSet::from_iter([range]))
  }

  /// The ids of the pending faces `children` request in a block styled by `css`, with Geist Mono
  /// registered ahead of `faces`.
  fn requested(
    faces: impl IntoIterator<Item = PendingFontFace>,
    children: impl Into<Vec<Node>>,
    css: &str,
  ) -> Vec<usize> {
    let mut fonts = Fonts::default();

    register(&mut fonts, GEIST_MONO, "Geist Mono");
    requested_with(fonts, faces, children, css)
  }

  /// [`requested`] with `fonts` registered ahead of `faces`.
  fn requested_with(
    mut fonts: Fonts,
    faces: impl IntoIterator<Item = PendingFontFace>,
    children: impl Into<Vec<Node>>,
    css: &str,
  ) -> Vec<usize> {
    for face in faces {
      fonts.register_pending(face);
    }

    render_with_fonts(
      &fonts,
      Node::container(children).with_class_name("container"),
      css,
    )
    .needs()
    .faces
    .into_iter()
    .collect()
  }

  /// The characters of `probe` whose own pending face `children` request in a block styled by
  /// `css`, every face in the block's only family.
  fn shaped(probe: &str, children: impl Into<Vec<Node>>, css: &str) -> String {
    let probe: Vec<char> = probe.chars().collect();
    let faces = probe.iter().enumerate().map(|(id, &character)| {
      pending(
        id,
        "Probe",
        400.0,
        UnicodeRange::new(character as u32, character as u32),
      )
    });
    let css = format!(".container {{ font-family: Probe }} {css}");
    let mut shaped: Vec<char> = requested_with(Fonts::default(), faces, children, &css)
      .into_iter()
      .map(|id| probe[id])
      .collect();

    shaped.sort_unstable();
    shaped.into_iter().collect()
  }

  #[test]
  fn requests_only_the_first_family_covering_a_character() {
    let faces = requested(
      [pending(0, "A", 400.0, LATIN), pending(1, "B", 400.0, LATIN)],
      [Node::text("Hello")],
      ".container { font-family: A, B }",
    );

    assert_eq!(faces, [0]);
  }

  #[test]
  fn requests_a_later_family_for_a_character_only_it_covers() {
    let faces = requested(
      [
        pending(0, "A", 400.0, LATIN),
        pending(1, "B", 400.0, CYRILLIC),
      ],
      [Node::text("HiЖ")],
      ".container { font-family: A, B }",
    );

    assert_eq!(faces, [0, 1]);
  }

  #[test]
  fn requests_the_weight_the_text_matches() {
    let faces = [pending(0, "A", 400.0, LATIN), pending(1, "A", 700.0, LATIN)];

    assert_eq!(
      requested(
        faces.clone(),
        [Node::text("a")],
        ".container { font-family: A }"
      ),
      [0]
    );
    assert_eq!(
      requested(
        faces,
        [Node::text("a"), Node::text("b").with_class_name("bold")],
        ".container { font-family: A } .bold { font-weight: 700 }",
      ),
      [0, 1]
    );
  }

  #[test]
  fn a_pending_weight_competes_with_a_registered_one() {
    let mut fonts = Fonts::default();

    register(&mut fonts, POPPINS_REGULAR, "A");

    assert_eq!(
      requested_with(
        fonts,
        [pending(0, "A", 700.0, LATIN)],
        [Node::text("a"), Node::text("b").with_class_name("bold")],
        ".container { font-family: A } .bold { font-weight: 700 }",
      ),
      [0]
    );
  }

  #[test]
  fn requests_nothing_a_loaded_font_earlier_in_the_stack_draws() {
    let faces = requested(
      [pending(0, "A", 400.0, LATIN)],
      [Node::text("Hello")],
      ".container { font-family: 'Geist Mono', A }",
    );

    assert!(faces.is_empty());
  }

  #[test]
  fn requests_the_unicode_range_subsets_the_text_falls_in() {
    let faces = || {
      [
        pending(0, "A", 400.0, LATIN),
        pending(1, "A", 400.0, CYRILLIC),
      ]
    };
    let css = ".container { font-family: A }";

    assert_eq!(requested(faces(), [Node::text("Ж")], css), [1]);
    assert_eq!(requested(faces(), [Node::text("Hi Ж")], css), [0, 1]);
  }

  #[test]
  fn requests_a_fallback_family_s_face_after_the_stack() {
    let faces = requested(
      [pending(0, "A", 400.0, CJK)],
      [Node::text("a中")],
      ".container { font-family: 'Geist Mono' }",
    );

    assert_eq!(faces, [0]);
  }

  #[test]
  fn requests_the_face_of_an_svg_image_s_text_font() {
    let faces = requested(
      [
        pending(0, "A", 400.0, CYRILLIC),
        pending(1, "B", 400.0, CYRILLIC),
      ],
      [Node::image(
        "<svg xmlns='http://www.w3.org/2000/svg'><text font-family='B'>Ж</text></svg>",
      )],
      ".container { font-family: A }",
    );

    assert_eq!(faces, [1]);
  }

  #[test]
  fn requests_nothing_without_pending_faces() {
    let needs = render_tree([Node::text("Hello Ж")], "").needs();

    assert!(needs.faces.is_empty());
  }

  const CSS: &str = "
    .hidden { display: none; background-image: url(https://example.com/hidden.png) }
    .bg { background-image: url(https://example.com/bg.png), url(data:image/png;base64,AA) }
    .bg::before { content: 'x' url(https://example.com/before.png) }
    .mask { mask-image: url('https://example.com/mask.png') }
    .upper { text-transform: uppercase }
    .item { display: list-item; list-style: decimal inside }
  ";

  #[test]
  fn reads_nothing_under_a_hidden_root() {
    let needs = render_tree([Node::text("a")], ".container { display: none }").needs();

    assert_eq!(needs, RenderNeeds::default());
  }

  #[test]
  fn reads_a_marker_image_only_for_a_list_item() {
    let needs = render_tree(
      [Node::container([Node::text("a")]).with_class_name("plain")],
      ".container { list-style-image: url(https://example.com/marker.png) }",
    )
    .needs();

    assert!(needs.images.is_empty());
  }

  #[test]
  fn reads_images_an_animation_switches_to() {
    let needs = render_tree(
      [Node::container([]).with_class_name("spin")],
      "@keyframes swap { to { background-image: url(https://example.com/later.png) } }
       @keyframes unused { to { background-image: url(https://example.com/unused.png) } }
       .spin { animation: swap 1s }",
    )
    .needs();

    assert_eq!(
      needs.images.into_iter().collect::<Vec<_>>(),
      ["https://example.com/later.png"]
    );
  }

  #[test]
  fn capitalizes_only_where_a_word_starts() {
    let shaped = shaped(
      " abcdefABCDEF",
      [span("ab"), span("cd"), span(" ef")],
      ".span { text-transform: capitalize }",
    );

    assert_eq!(shaped, " AEbcdf");
  }

  #[test]
  fn reads_a_nested_inline_with_the_context_around_it() {
    let shaped = shaped(
      "abcdABCD",
      [
        span("ab"),
        Node::container([Node::text("cd")]).with_class_name("span"),
      ],
      ".span { text-transform: capitalize }",
    );

    assert_eq!(shaped, "Abcd");
  }

  #[test]
  fn reads_a_floated_text_in_an_inline_context() {
    let shaped = shaped(
      "aq",
      [span("a"), Node::text("q").with_class_name("float")],
      ".float { float: left }",
    );

    assert_eq!(shaped, "aq");
  }

  #[test]
  fn reads_full_width_transforms() {
    let shaped = shaped(
      "AＡ",
      [Node::text("A").with_class_name("wide")],
      ".wide { text-transform: full-width }",
    );

    assert_eq!(shaped, "Ａ");
  }

  #[test]
  fn reads_what_the_cascade_leaves() {
    let needs = render_tree(
      [
        Node::text("ab").with_class_name("upper"),
        Node::container([Node::text("z")]).with_class_name("hidden"),
        Node::container([]).with_class_name("bg"),
        Node::container([]).with_class_name("mask"),
        Node::container([Node::text("c")]).with_class_name("item"),
        Node::image("https://example.com/img.png"),
        Node::image("data:image/png;base64,AA"),
        Node::image("HTTPS://example.com/upper.png"),
        Node::image("/local.png"),
        Node::image("<svg xmlns='http://www.w3.org/2000/svg'/>"),
      ],
      CSS,
    )
    .needs();

    assert_eq!(
      needs.images.into_iter().collect::<Vec<_>>(),
      [
        "/local.png",
        "HTTPS://example.com/upper.png",
        "https://example.com/before.png",
        "https://example.com/bg.png",
        "https://example.com/img.png",
        "https://example.com/mask.png",
      ]
    );
  }

  #[test]
  fn shapes_what_the_cascade_leaves() {
    let shaped = shaped(
      " .1ABabcxz",
      [
        Node::text("ab").with_class_name("upper"),
        Node::container([Node::text("z")]).with_class_name("hidden"),
        Node::container([]).with_class_name("bg"),
        Node::container([Node::text("c")]).with_class_name("item"),
      ],
      CSS,
    );

    assert_eq!(shaped, " .1ABcx");
  }

  #[test]
  fn reads_the_text_an_inline_svg_draws() {
    let shaped = shaped(
      "QΩЖ",
      [Node::image(
        "<svg xmlns='http://www.w3.org/2000/svg'><title>Q</title><text>Ж<tspan>Ω</tspan></text></svg>",
      )],
      "",
    );

    assert_eq!(shaped, "ΩЖ");
  }

  #[test]
  fn reads_the_text_an_svg_data_uri_draws() {
    let chart = || [Node::container([]).with_class_name("chart")];
    let css = ".chart { width: 10px; height: 10px; background-image: url(\"data:image/svg+xml,<svg xmlns='http://www.w3.org/2000/svg'><text>Ж</text></svg>\") }";

    assert_eq!(shaped("Ж", chart(), css), "Ж");
    assert!(render_tree(chart(), css).needs().images.is_empty());
  }
}
