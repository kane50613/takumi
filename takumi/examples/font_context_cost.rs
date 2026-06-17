//! Find the minimum marginal cost of producing a per-render `Fonts`.
//!
//! Run: `cargo run --release --example font_context_cost`
//!
//! Strategies, given fonts are already DECODED once (the only expensive step,
//! ~20ms/woff2 — see git history of this file):
//!   - FORK:    clone a prebuilt base context
//!   - REBUILD: fresh default() + register N decoded fonts
//!   - ADD-1:   marginal cost of registering one more decoded font into a fork
//!   - REUSE:   look up a prebuilt context by set-hash (≈ a HashMap get)

use std::{
  collections::HashMap,
  path::{Path, PathBuf},
  sync::Arc,
  time::Instant,
};

use parley::fontique::{Blob, FontInfoOverride};
use takumi::base::resources::font::{FontResource, FontSource, Fonts};

fn repo_path(rel: &str) -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(rel)
}

fn time<F: FnMut()>(iters: u32, mut f: F) -> f64 {
  f();
  let start = Instant::now();
  for _ in 0..iters {
    f();
  }
  start.elapsed().as_secs_f64() * 1e6 / iters as f64 // mean µs/iter
}

fn main() {
  // HEAVIEST decoded font: NotoColorEmoji.ttf (9.9 MB, big charmap + color tables).
  let bytes =
    std::fs::read(repo_path("assets/fonts/noto-sans/NotoColorEmoji.ttf")).expect("read font");
  println!("heavy decoded font: {} KB\n", bytes.len() / 1024);
  let blob = Blob::new(Arc::new(bytes));

  // Distinct family names so each register adds a real family (no dedup collapse).
  let names: Vec<String> = (0..64).map(|i| format!("Fam {i}")).collect();

  let register = |ctx: &mut Fonts, i: usize| {
    ctx
      .load_and_store(
        FontResource::new(FontSource::Blob(blob.clone())).override_info(FontInfoOverride {
          family_name: Some(&names[i]),
          ..Default::default()
        }),
      )
      .expect("register decoded");
  };

  let build = |n: usize| {
    let mut ctx = Fonts::default();
    for i in 0..n {
      register(&mut ctx, i);
    }
    ctx
  };

  println!("decoded blob registered repeatedly with distinct family names\n");
  println!(
    "{:>5} | {:>14} | {:>16} | {:>10} | {:>14}",
    "fonts", "FORK clone µs", "REBUILD µs", "fork/font", "REBUILD/font"
  );
  println!("{}", "-".repeat(72));

  for &n in &[1usize, 3, 6, 12] {
    let base = build(n);
    let fork = time(3000, || {
      std::hint::black_box(base.clone());
    });
    let rebuild = time(20, || {
      std::hint::black_box(build(n));
    });
    println!(
      "{n:>5} | {fork:>14.3} | {rebuild:>16.2} | {:>10.3} | {:>14.2}",
      fork / n as f64,
      rebuild / n as f64
    );
  }

  // Decode cache: cold (first decode) vs warm (content-addressed hit) for a heavy CJK woff2.
  let woff2 = std::fs::read(repo_path(
    "assets/fonts/noto-sans/NotoSansTC-VariableFont_wght.woff2",
  ))
  .expect("read woff2");
  // The decode cache lives on the context; clones/forks share it (Arc).
  let cache_base = Fonts::default();
  let rebuild_woff2 = || {
    let mut ctx = cache_base.clone(); // shares cache_base's decode cache
    ctx
      .load_and_store(FontResource::new(woff2.as_slice()))
      .expect("decode+register");
    std::hint::black_box(ctx);
  };

  let cold = Instant::now();
  rebuild_woff2(); // first time → real woff2 decode, populates the shared cache
  let cold_us = cold.elapsed().as_secs_f64() * 1e6;
  let warm_us = time(1000, rebuild_woff2); // cache hit → no decode

  println!("\nheavy CJK woff2 ({} KB compressed):", woff2.len() / 1024);
  println!("  COLD rebuild (real decode) : {cold_us:>10.1} µs");
  println!(
    "  WARM rebuild (cache hit)   : {warm_us:>10.2} µs   ({:.0}x faster)",
    cold_us / warm_us
  );

  // Marginal cost of adding ONE decoded font on top of a fork (base = 6).
  let base6 = build(6);
  let fork6 = time(5000, || {
    std::hint::black_box(base6.clone());
  });
  let fork_plus_1 = time(2000, || {
    let mut c = base6.clone();
    register(&mut c, 6);
    std::hint::black_box(c);
  });

  // REUSE floor: hash the requested set + HashMap lookup of a prebuilt context.
  let mut cache: HashMap<u64, Fonts> = HashMap::new();
  cache.insert(0xABCD, build(6));
  let reuse = time(50_000, || {
    let key = std::hint::black_box(0xABCDu64);
    std::hint::black_box(cache.contains_key(&key));
  });

  println!("\n--- marginal costs ---");
  println!("FORK base(6) clone          : {fork6:>10.3} µs");
  println!(
    "ADD-1 decoded onto fork(6)  : {:>10.3} µs   (register-one ≈ {:.3} µs)",
    fork_plus_1,
    fork_plus_1 - fork6
  );
  println!("REUSE prebuilt by set-hash  : {reuse:>10.4} µs   (HashMap get)");
}
