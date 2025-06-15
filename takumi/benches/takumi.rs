use std::io::Cursor;

use criterion::{Criterion, criterion_group, criterion_main};
use image::ImageFormat;
use takumi::{
  color::{Color, ColorInput, Gradient},
  context::GlobalContext,
  impl_node_enum,
  node::{
    ContainerNode, TextNode,
    style::{FontWeight, InheritableStyle, Style, TextAlign},
  },
  render::{ImageRenderer, Viewport},
};

#[derive(Debug, Clone)]
enum NodeKind {
  Container(ContainerNode<NodeKind>),
  Text(TextNode),
}

impl_node_enum!(NodeKind, Container => ContainerNode<NodeKind>, Text => TextNode);

fn render_scenarios(c: &mut Criterion) {
  let mut group = c.benchmark_group("render_scenarios");

  group.bench_function("gradient_background", |b| {
    b.iter(|| {
      let context = GlobalContext::default();
      let mut renderer: ImageRenderer<NodeKind> = ImageRenderer::new(Viewport::new(1200, 630));
      renderer.construct_taffy_tree(
        ContainerNode {
          style: Style {
            width: 1200.0.into(),
            height: 630.0.into(),
            background_color: Some(ColorInput::Gradient(Gradient {
              stops: vec![Color::Rgb(242, 102, 223), Color::Rgb(157, 201, 106)],
              angle: 45.0,
            })),
            ..Default::default()
          },
          children: None,
        }
        .into(),
        &context,
      );
      let image = renderer.draw(&context).unwrap();
      let mut buffer = Vec::new();
      let mut cursor = Cursor::new(&mut buffer);
      image.write_to(&mut cursor, ImageFormat::WebP).unwrap();
    })
  });

  // Empty container baseline
  group.bench_function("empty_container", |b| {
    b.iter(|| {
      let context = GlobalContext::default();
      let mut renderer: ImageRenderer<NodeKind> = ImageRenderer::new(Viewport::new(1200, 630));
      renderer.construct_taffy_tree(
        ContainerNode {
          style: Style {
            width: 1200.0.into(),
            height: 630.0.into(),
            ..Default::default()
          },
          children: None,
        }
        .into(),
        &context,
      );
      let image = renderer.draw(&context).unwrap();
      let mut buffer = Vec::new();
      let mut cursor = Cursor::new(&mut buffer);
      image.write_to(&mut cursor, ImageFormat::WebP).unwrap();
    })
  });

  // Nested containers
  group.bench_function("nested_containers", |b| {
    b.iter(|| {
      let context = GlobalContext::default();
      let mut renderer: ImageRenderer<NodeKind> = ImageRenderer::new(Viewport::new(1200, 630));
      renderer.construct_taffy_tree(
        ContainerNode {
          style: Style {
            width: 1200.0.into(),
            height: 630.0.into(),
            background_color: Some(Color::Rgb(240, 240, 240).into()),
            ..Default::default()
          },
          children: Some(vec![
            ContainerNode {
              style: Style {
                width: 400.0.into(),
                height: 400.0.into(),
                background_color: Some(Color::Rgb(200, 200, 200).into()),
                ..Default::default()
              },
              children: Some(vec![
                ContainerNode {
                  style: Style {
                    width: 200.0.into(),
                    height: 200.0.into(),
                    background_color: Some(Color::Rgb(160, 160, 160).into()),
                    ..Default::default()
                  },
                  children: None,
                }
                .into(),
              ]),
            }
            .into(),
          ]),
        }
        .into(),
        &context,
      );
      let image = renderer.draw(&context).unwrap();
      let mut buffer = Vec::new();
      let mut cursor = Cursor::new(&mut buffer);
      image.write_to(&mut cursor, ImageFormat::WebP).unwrap();
    })
  });

  group.finish();
}

fn render_performance_analysis(c: &mut Criterion) {
  let mut group = c.benchmark_group("performance_analysis");

  // Complex layout for detailed analysis
  let complex_layout = || {
    ContainerNode {
      style: Style {
        width: 1200.0.into(),
        height: 630.0.into(),
        background_color: Some(Color::Rgb(240, 240, 240).into()),
        ..Default::default()
      },
      children: Some(vec![
        TextNode {
          style: Style {
            width: 800.0.into(),
            height: 100.0.into(),
            inheritable_style: InheritableStyle {
              font_size: Some(48.0.into()),
              color: Some(Color::Rgb(0, 0, 0).into()),
              text_align: Some(TextAlign::Center),
              font_weight: Some(FontWeight(700)),
              ..Default::default()
            },
            ..Default::default()
          },
          text: "Complex Layout Benchmark".to_string(),
        }
        .into(),
        ContainerNode {
          style: Style {
            width: 1000.0.into(),
            height: 400.0.into(),
            background_color: Some(Color::Rgb(200, 200, 200).into()),
            ..Default::default()
          },
          children: Some(vec![
            TextNode {
              style: Style {
                width: 400.0.into(),
                height: 100.0.into(),
                inheritable_style: InheritableStyle {
                  font_size: Some(24.0.into()),
                  color: Some(Color::Rgb(50, 50, 50).into()),
                  text_align: Some(TextAlign::Left),
                  ..Default::default()
                },
                ..Default::default()
              },
              text: "This is a complex layout with multiple nested elements and text nodes."
                .to_string(),
            }
            .into(),
            ContainerNode {
              style: Style {
                width: 200.0.into(),
                height: 200.0.into(),
                background_color: Some(Color::Rgb(160, 160, 160).into()),
                ..Default::default()
              },
              children: None,
            }
            .into(),
          ]),
        }
        .into(),
      ]),
    }
    .into()
  };

  // Full pipeline benchmark
  group.bench_function("full_pipeline", |b| {
    b.iter(|| {
      let context = GlobalContext::default();
      let mut renderer: ImageRenderer<NodeKind> = ImageRenderer::new(Viewport::new(1200, 630));
      renderer.construct_taffy_tree(complex_layout(), &context);
      let image = renderer.draw(&context).unwrap();
      let mut buffer = Vec::new();
      let mut cursor = Cursor::new(&mut buffer);
      image.write_to(&mut cursor, ImageFormat::WebP).unwrap();
    })
  });

  // Tree creation step
  group.bench_function("tree_creation", |b| {
    b.iter(|| {
      let context = GlobalContext::default();
      let mut renderer = ImageRenderer::new(Viewport::new(1200, 630));
      renderer.construct_taffy_tree(complex_layout(), &context);
    })
  });

  // Drawing step
  group.bench_function("image_drawing", |b| {
    let context = GlobalContext::default();
    let mut renderer = ImageRenderer::new(Viewport::new(1200, 630));
    renderer.construct_taffy_tree(complex_layout(), &context);

    b.iter(|| renderer.draw(&context).unwrap())
  });

  // Encoding step
  group.bench_function("webp_encoding", |b| {
    let context = GlobalContext::default();
    let mut renderer = ImageRenderer::new(Viewport::new(1200, 630));
    renderer.construct_taffy_tree(complex_layout(), &context);
    let image = renderer.draw(&context).unwrap();

    b.iter(|| {
      let mut buffer = Vec::new();
      let mut cursor = Cursor::new(&mut buffer);
      image.write_to(&mut cursor, ImageFormat::WebP).unwrap();
    })
  });

  group.finish();
}

criterion_group!(benches, render_scenarios, render_performance_analysis);
criterion_main!(benches);
