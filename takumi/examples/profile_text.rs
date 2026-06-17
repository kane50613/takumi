use parley::{GenericFamily, fontique::FontInfoOverride};
use std::hint::black_box;
use takumi::base::{
  Fonts,
  layout::{Viewport, node::Node},
  resources::font::FontResource,
};
use takumi::raster::{RenderOptions, render};

const ITERS: usize = 80;

const LONG_TEXT: &str = "Typography is the art and technique of arranging type to make written language legible, \
   readable and appealing when displayed. The arrangement of type involves selecting typefaces, \
   point sizes, line lengths, line-spacing and letter-spacing, and adjusting the space between \
   pairs of letters. The term typography is also applied to the style, arrangement, and \
   appearance of the letters, numbers, and symbols created by the process. Type design is a \
   closely related craft, sometimes considered part of typography; most typographers do not \
   design typefaces, and some type designers do not consider themselves typographers.";

fn load_global() -> Fonts {
  let mut g = Fonts::default();
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

fn many_runs() -> Node {
  let palette = [
    "text-gray-900",
    "text-red-600",
    "text-green-600",
    "text-blue-600",
    "text-purple-600",
    "text-orange-600",
  ];
  let weights = [
    "font-normal",
    "font-medium",
    "font-semibold",
    "font-bold",
    "font-extrabold",
  ];
  let mut spans: Vec<Node> = Vec::new();
  for (i, word) in LONG_TEXT.split_whitespace().enumerate() {
    let color = palette[i % palette.len()];
    let weight = weights[i % weights.len()];
    let tw = format!("flex text-[26px] {color} {weight}");
    spans.push(Node::text(format!("{word} ")).with_tw(tw.parse().unwrap()));
  }
  Node::container(spans).with_tw("flex flex-row w-full h-full p-12 bg-white".parse().unwrap())
}

fn main() {
  let g = load_global();
  for _ in 0..ITERS {
    let opts = RenderOptions::builder()
      .viewport(Viewport::new((1200, 630)))
      .node(many_runs())
      .fonts(&g)
      .build();
    black_box(render(opts).unwrap());
  }
}
