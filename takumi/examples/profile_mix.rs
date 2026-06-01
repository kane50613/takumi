use parley::{GenericFamily, fontique::FontInfoOverride};
use std::{env, hint::black_box};
use takumi::base::{
  FontContext,
  layout::{Viewport, node::Node},
  resources::font::FontResource,
};
use takumi::raster::{RenderOptions, render};

const LONG_TEXT: &str = "Typography is the art and technique of arranging type to make written language legible, \
   readable and appealing when displayed. The arrangement of type involves selecting typefaces, \
   point sizes, line lengths, line-spacing and letter-spacing, and adjusting the space between \
   pairs of letters.";

fn load_global() -> FontContext {
  let mut g = FontContext::default();
  let regular: &[u8] = include_bytes!("../../assets/fonts/geist/Geist[wght].woff2");
  g.load_and_store(
    FontResource::new(regular.to_vec())
      .override_info(FontInfoOverride {
        family_name: Some("Geist"),
        ..Default::default()
      })
      .generic_family(GenericFamily::SansSerif),
  )
  .unwrap();
  g
}

fn blur_3xl() -> Node {
  Node::container([]).with_tw("w-[256px] h-[256px] bg-white blur-3xl".parse().unwrap())
}

fn shadow_2xl() -> Node {
  Node::container([]).with_tw("w-[256px] h-[256px] bg-white shadow-2xl".parse().unwrap())
}

fn drop_shadow_2xl() -> Node {
  Node::container([]).with_tw(
    "w-[256px] h-[256px] bg-white drop-shadow-2xl"
      .parse()
      .unwrap(),
  )
}

fn shadowed_decorated() -> Node {
  Node::container([Node::text(LONG_TEXT.to_string()).with_tw(
    "text-[36px] font-bold text-gray-900 underline line-through"
      .parse()
      .unwrap(),
  )])
  .with_tw("flex w-full h-full p-12 bg-white".parse().unwrap())
}

fn clipped_text_paragraph() -> Node {
  Node::container([
    Node::text(LONG_TEXT.to_string()).with_tw(
      "text-[36px] font-extrabold bg-clip-text text-transparent bg-gradient-to-r from-red-500 via-yellow-400 to-blue-600".parse().unwrap(),
    ),
  ])
  .with_tw("flex w-full h-full p-12 items-center justify-center bg-white".parse().unwrap())
}

fn long_paragraph() -> Node {
  Node::container([
    Node::text(LONG_TEXT.to_string()).with_tw("text-[28px] text-gray-900".parse().unwrap())
  ])
  .with_tw("flex w-full h-full p-12 bg-white".parse().unwrap())
}

fn render_node(g: &FontContext, node: Node, width: u32, height: u32) {
  let opts = RenderOptions::builder()
    .viewport(Viewport::new((width, height)))
    .node(node)
    .font_context(g)
    .build();
  black_box(render(opts).unwrap());
}

fn main() {
  let g = load_global();
  let fixture = env::args().nth(1).unwrap_or_else(|| "blur".to_string());
  let iters: usize = env::args()
    .nth(2)
    .and_then(|s| s.parse().ok())
    .unwrap_or(160);
  match fixture.as_str() {
    "blur" => {
      for _ in 0..iters {
        render_node(&g, blur_3xl(), 512, 512);
      }
    }
    "shadow" => {
      for _ in 0..iters {
        render_node(&g, shadow_2xl(), 512, 512);
      }
    }
    "drop_shadow" => {
      for _ in 0..iters {
        render_node(&g, drop_shadow_2xl(), 512, 512);
      }
    }
    "shadowed_decorated" => {
      for _ in 0..iters {
        render_node(&g, shadowed_decorated(), 1200, 630);
      }
    }
    "clipped_text" => {
      for _ in 0..iters {
        render_node(&g, clipped_text_paragraph(), 1200, 630);
      }
    }
    "long_paragraph" => {
      for _ in 0..iters {
        render_node(&g, long_paragraph(), 1200, 630);
      }
    }
    other => panic!("unknown fixture: {other}"),
  }
}
