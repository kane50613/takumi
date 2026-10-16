use std::{cell::Cell, hint::black_box};

use criterion::{Criterion, criterion_group, criterion_main};
use takumi::{
  prelude::{FontOverride, FontResource, Fonts, GenericFamily, Node, RenderOptions, Viewport},
  render,
  unstable::base::resources::shape_cache::set_shape_cache_max_bytes,
};

mod common;

const DPR: f32 = 1.5;
const SCREEN_HEIGHT: u32 = 1440;
const SIDEBAR_WIDTH: u32 = 272;
const CALENDAR_WIDTH: u32 = 60;
const CACHE_BUDGET: usize = 4 << 20;

const MUTED: &str = "text-[#8a8fa3]";
const FOREGROUND: &str = "text-[#d8dcec]";
const GREEN: &str = "text-[#a6e3a1]";
const BLUE: &str = "text-[#89b4fa]";

struct Workspace {
  key: &'static str,
  title: &'static str,
  subtitle: &'static str,
  focused: bool,
  checkbox: Option<bool>,
}

const WORKSPACES: [Workspace; 6] = [
  Workspace {
    key: "1",
    title: "Refactor billing export - nightly job",
    subtitle: "billing-service#212",
    focused: true,
    checkbox: Some(false),
  },
  Workspace {
    key: "2",
    title: "Investigate flaky checkout test",
    subtitle: "storefront#87",
    focused: false,
    checkbox: Some(false),
  },
  Workspace {
    key: "3",
    title: "Add retry budget to the sync worker",
    subtitle: "sync-worker#154",
    focused: false,
    checkbox: Some(true),
  },
  Workspace {
    key: "4",
    title: "Docs",
    subtitle: "Reading the migration guide",
    focused: false,
    checkbox: None,
  },
  Workspace {
    key: "5",
    title: "Terminal",
    subtitle: "~/projects",
    focused: false,
    checkbox: None,
  },
  Workspace {
    key: "6",
    title: "Music",
    subtitle: "Spotify",
    focused: false,
    checkbox: None,
  },
];

const UP_NEXT: [&str; 4] = [
  "Rate limit the public search endpoint",
  "Upgrade the payments SDK to v5",
  "Write the onboarding email templates",
  "Audit unused feature flags",
];

const DONE: [&str; 6] = [
  "Fix timezone drift in reports",
  "Cache the pricing lookup",
  "Remove the legacy login route",
  "Bump the base image",
  "Tidy the CI matrix",
  "Rename the metrics namespace",
];

const WIP: [(&str, u32, u32); 5] = [
  ("billing-service", 2, 1),
  ("storefront", 1, 0),
  ("sync-worker", 0, 3),
  ("design-tokens", 1, 0),
  ("infra-playbooks", 0, 2),
];

const REVIEW: [(&str, u32); 3] = [
  ("billing-service", 1),
  ("search-index", 2),
  ("mobile-app", 1),
];

const CONTAINERS: [&str; 3] = ["billing-service", "storefront", "sync-worker"];

const AGENDA: [(&str, u32, u32, &str); 4] = [
  ("Standup", 9 * 60, 9 * 60 + 30, "#818cf8"),
  ("Design review", 11 * 60, 12 * 60 + 30, "#34d399"),
  ("1:1 sync", 14 * 60, 14 * 60 + 45, "#fbbf24"),
  ("Planning", 16 * 60, 17 * 60 + 30, "#f472b6"),
];

fn fonts() -> Fonts {
  let mut fonts = Fonts::default();
  let mono: &[u8] =
    include_bytes!("../../assets/fonts/google-sans-code/GoogleSansCode-VariableFont_wght.woff2");
  fonts
    .register(
      FontResource::new(mono.to_vec())
        .override_info(FontOverride {
          family_name: Some("Google Sans Code".into()),
          ..Default::default()
        })
        .generic_family(GenericFamily::SANS_SERIF),
    )
    .unwrap();
  let emoji: &[u8] = include_bytes!("../../assets/fonts/twemoji/TwemojiMozilla-colr.woff2");
  fonts
    .register(
      FontResource::new(emoji.to_vec())
        .override_info(FontOverride {
          family_name: Some("Twemoji Mozilla".into()),
          ..Default::default()
        })
        .generic_family(GenericFamily::EMOJI),
    )
    .unwrap();
  fonts
}

fn element(class: &str, children: impl Into<Vec<Node>>) -> Node {
  Node::container(children).with_tw(class.parse().unwrap())
}

fn span(class: &str, text: impl Into<String>) -> Node {
  element(&format!("inline {class}"), [Node::text(text)])
}

fn truncated(text: &str, limit: usize) -> String {
  if text.chars().count() > limit {
    let head: String = text.chars().take(limit - 1).collect();
    format!("{head}…")
  } else {
    text.to_string()
  }
}

fn section(children: Vec<Node>) -> Node {
  element("flex flex-col w-full gap-[4px] px-3 py-[8px]", children)
}

fn workspace_row(workspace: &Workspace) -> Node {
  let title_class = if workspace.focused {
    format!("text-[12px] font-bold truncate {FOREGROUND}")
  } else {
    format!("text-[12px] truncate {MUTED}")
  };
  let mut top = Vec::new();

  if let Some(done) = workspace.checkbox {
    let (class, mark) = if done {
      (
        format!("text-[11px] font-bold flex-shrink-0 {GREEN}"),
        "[x]",
      )
    } else {
      (
        format!("text-[11px] font-bold flex-shrink-0 {MUTED}"),
        "[ ]",
      )
    };
    top.push(span(&class, mark));
  }

  top.push(element(
    "flex-1 flex items-center min-w-0",
    [span(&title_class, workspace.title)],
  ));
  top.push(element(
    "flex flex-col items-center justify-center flex-shrink-0 w-[26px] py-[2px] rounded-md",
    [span(
      &format!("text-[11px] font-bold {FOREGROUND}"),
      workspace.key,
    )],
  ));

  element(
    "flex flex-row items-stretch rounded-md w-full px-3 py-[6px]",
    [element(
      "flex flex-col w-full gap-[3px]",
      [
        element("flex flex-row items-center gap-[6px] w-full min-w-0", top),
        element(
          "flex flex-row items-center gap-[6px] w-full min-w-0",
          [span(
            &format!("text-[10px] truncate min-w-0 {MUTED}"),
            workspace.subtitle,
          )],
        ),
      ],
    )],
  )
}

fn task_chip(label: &str) -> Node {
  element(
    "flex flex-row items-center gap-[6px] px-2 py-[2px] w-full min-w-0",
    [
      element("w-[5px] h-[5px] rounded-full flex-shrink-0", []),
      span(&format!("text-[10px] truncate min-w-0 {MUTED}"), label),
    ],
  )
}

fn task_cards() -> Node {
  let mut up_next = vec![span(
    &format!("text-[9px] font-bold px-2 pb-[2px] {MUTED}"),
    "UP NEXT",
  )];
  up_next.extend(UP_NEXT.iter().map(|label| task_chip(label)));
  let mut done = vec![span(
    &format!("text-[9px] font-bold px-2 pb-[2px] {MUTED}"),
    "DONE · LAST 8H",
  )];
  done.extend(DONE.iter().map(|label| task_chip(label)));

  element(
    "flex flex-col w-full gap-[6px] px-1 py-[8px] border-t",
    [
      element("flex flex-col w-full", up_next),
      element("flex flex-col w-full", done),
    ],
  )
}

fn github_card(tick: u64) -> Node {
  let mut rows = vec![element(
    "flex flex-row items-baseline",
    [
      span(&format!("flex-1 text-[10px] {MUTED}"), "GITHUB WIP"),
      span(&format!("w-[24px] text-right text-[8px] {MUTED}"), "PR"),
      span(&format!("w-[24px] text-right text-[8px] {MUTED}"), "tsk"),
    ],
  )];

  for (index, (repo, prs, issues)) in WIP.iter().enumerate() {
    let prs = if index == 0 {
      prs + (tick % 4) as u32
    } else {
      *prs
    };
    let count = |value: u32| {
      if value > 0 {
        value.to_string()
      } else {
        "—".to_string()
      }
    };
    rows.push(element(
      "flex flex-row items-baseline",
      [
        span(
          &format!("flex-1 text-[11px] {FOREGROUND}"),
          truncated(repo, 22),
        ),
        span(
          &format!("w-[24px] text-right text-[11px] {FOREGROUND}"),
          count(prs),
        ),
        span(
          &format!("w-[24px] text-right text-[11px] {FOREGROUND}"),
          count(*issues),
        ),
      ],
    ));
  }

  section(rows)
}

fn review_card() -> Node {
  let mut rows = vec![element(
    "flex flex-row items-baseline",
    [
      span(&format!("flex-1 text-[10px] {MUTED}"), "REVIEW"),
      span(&format!("w-[24px] text-right text-[8px] {MUTED}"), "PR"),
    ],
  )];

  for (repo, count) in REVIEW {
    rows.push(element(
      "flex flex-row items-baseline",
      [
        span(
          &format!("flex-1 text-[11px] {FOREGROUND}"),
          truncated(repo, 22),
        ),
        span(
          &format!("w-[24px] text-right text-[11px] {FOREGROUND}"),
          count.to_string(),
        ),
      ],
    ));
  }

  section(rows)
}

fn devcontainer_card(tick: u64) -> Node {
  let mut rows = vec![element(
    "flex flex-row items-baseline",
    [
      span(&format!("flex-1 text-[10px] {MUTED}"), "DEVCONTAINERS"),
      span(&format!("w-[44px] text-right text-[8px] {MUTED}"), "mem"),
    ],
  )];

  for (index, name) in CONTAINERS.iter().enumerate() {
    let memory = format!("{:.1}G", 1.2 + index as f64 * 0.7 + (tick % 5) as f64 * 0.1);
    rows.push(element(
      "flex flex-row items-baseline",
      [
        span(&format!("flex-1 text-[11px] {BLUE}"), truncated(name, 22)),
        span(&format!("w-[44px] text-right text-[11px] {GREEN}"), memory),
      ],
    ));
  }

  section(rows)
}

fn usage_card(tick: u64) -> Node {
  let accounts = [
    ("personal", 38 + tick % 9, "2h"),
    ("work", 71 + tick % 7, "4d"),
  ];
  let rows = accounts.iter().map(|(label, percent, resets)| {
    element(
      "flex flex-col gap-[6px]",
      [
        element(
          &format!("inline text-[10px] {MUTED}"),
          [Node::text("Claude · "), Node::text(*label)],
        ),
        element(
          "flex flex-row items-baseline justify-between",
          [
            span(
              &format!("text-[15px] font-bold {GREEN}"),
              format!("{percent}%"),
            ),
            element(
              &format!("inline text-[10px] {MUTED}"),
              [Node::text("resets "), Node::text(*resets)],
            ),
          ],
        ),
        element("w-full h-[4px] rounded-full", []),
      ],
    )
  });

  section(rows.collect())
}

fn weather_card() -> Node {
  section(vec![element(
    "flex flex-row items-baseline justify-between w-full",
    [
      span(&format!("text-[18px] {FOREGROUND}"), "☀️ 21°"),
      element(
        "flex flex-col items-end",
        [
          span(&format!("text-[10px] {MUTED}"), "Clear, light wind"),
          span(&format!("text-[10px] {MUTED}"), "H 24° · L 14°"),
        ],
      ),
    ],
  )])
}

fn volume_card(tick: u64) -> Node {
  let volume = 30 + (tick * 5) % 60;

  section(vec![element(
    "flex flex-row items-baseline justify-between w-full",
    [
      span(&format!("text-[10px] {MUTED}"), "Volume"),
      span(&format!("text-[10px] {MUTED}"), format!("{volume}%")),
    ],
  )])
}

fn clock_card(tick: u64) -> Node {
  let cell = |label: &str, value: String| {
    element(
      "flex-1 flex flex-col gap-1",
      [
        span(&format!("text-[10px] {MUTED}"), label),
        span(&format!("text-[14px] {FOREGROUND}"), value),
      ],
    )
  };

  element(
    "flex flex-row w-full px-3 py-[8px]",
    [
      cell("TIME", clock(tick)),
      cell("BATTERY", format!("{}% ⚡", 62 + tick % 30)),
    ],
  )
}

fn clock(tick: u64) -> String {
  format!("{:02}:{:02}", 10 + (tick / 60) % 12, tick % 60)
}

fn sidebar(tick: u64) -> Node {
  element(
    "flex flex-col w-full h-full pl-[10px] pr-[16px] py-[10px] bg-[#1e1e2e]",
    [
      element(
        "flex-1 min-h-0 flex flex-col w-full overflow-hidden",
        [
          element(
            "flex flex-col gap-[2px] w-full",
            WORKSPACES.iter().map(workspace_row).collect::<Vec<_>>(),
          ),
          task_cards(),
        ],
      ),
      element(
        "flex flex-col w-full",
        [
          github_card(tick),
          review_card(),
          devcontainer_card(tick),
          weather_card(),
          usage_card(tick),
          volume_card(tick),
          clock_card(tick),
        ],
      ),
    ],
  )
}

const DAY_START: u32 = 6 * 60;
const DAY_END: u32 = 22 * 60;

fn calendar(tick: u64) -> Node {
  let now = 10 * 60 + (tick % 60) as u32;
  let mut boundaries: Vec<u32> = (DAY_START..=DAY_END).step_by(30).collect();

  for (_, start, end, _) in AGENDA {
    boundaries.extend([start, end]);
  }
  boundaries.push(now);
  boundaries.sort_unstable();
  boundaries.dedup();

  let mut segments = Vec::new();

  for pair in boundaries.windows(2) {
    let (start, end) = (pair[0], pair[1]);
    let event = AGENDA
      .iter()
      .find(|(_, from, to, _)| *from <= start && *to >= end);
    let is_now = start == now;
    let is_hour = start % 60 == 0;
    let is_half = !is_hour && start % 30 == 0;

    let mut class = format!(
      "flex flex-col shrink-0 basis-0 grow-[{}] overflow-hidden",
      end - start
    );
    class.push_str(match (event, is_now, is_hour, is_half) {
      (_, true, _, _) => " w-full border-t-2 border-[#ef4444] min-h-[18px]",
      (_, _, true, _) => " w-full border-t border-[#3a3d52]",
      (None, _, _, true) => " w-[30%] border-t border-[#2c2f42]",
      _ => " w-full",
    });

    let mut children = Vec::new();

    if let Some((title, from, _, color)) = event {
      class.push_str(&format!(
        " w-full bg-[{color}33] border-l-2 border-[{color}]"
      ));
      if start == (*from).max(DAY_START) {
        children.push(span(
          &format!("text-[8px] leading-none pl-[3px] pt-[2px] {FOREGROUND}"),
          *title,
        ));
      }
    } else if is_now {
      children.push(span(
        "text-[9px] leading-none pl-[3px] pt-[1px] text-[#ef4444]",
        format!("{:02}:{:02}", now / 60, now % 60),
      ));
    } else if is_hour {
      children.push(span(
        &format!("text-[9px] leading-none pl-[3px] pt-[3px] {MUTED}"),
        (start / 60).to_string(),
      ));
    }

    segments.push(element(&class, children));
  }

  element(
    "flex flex-col h-full w-full py-4 px-1 bg-[#1e1e2e]",
    [
      element(
        "flex flex-col w-full flex-1 min-h-0 overflow-hidden",
        segments,
      ),
      element(
        "w-full flex flex-col items-center pt-[6px] pb-[4px]",
        [
          span(
            &format!("text-[8px] font-bold leading-none {FOREGROUND}"),
            "Oct",
          ),
          span(
            &format!("text-[12px] font-bold leading-none pt-[2px] {FOREGROUND}"),
            "15",
          ),
        ],
      ),
    ],
  )
}

fn render_panel(fonts: &Fonts, node: Node, width: u32) {
  let options = RenderOptions::builder()
    .viewport(Viewport::new((width, SCREEN_HEIGHT)).with_device_pixel_ratio(DPR))
    .node(node)
    .fonts(fonts)
    .build();

  black_box(render(options).unwrap());
}

fn render_frame(fonts: &Fonts, tick: u64) {
  let physical = |css: u32| (css as f32 * DPR) as u32;

  render_panel(fonts, sidebar(tick), physical(SIDEBAR_WIDTH));
  render_panel(fonts, calendar(tick), physical(CALENDAR_WIDTH));
}

fn bench_panels(c: &mut Criterion) {
  let fonts = fonts();
  let mut group = c.benchmark_group("shape_cache_panels");

  for (name, budget) in [("cache_off", 0), ("cache_on", CACHE_BUDGET)] {
    let tick = Cell::new(0u64);

    group.bench_function(name, |b| {
      set_shape_cache_max_bytes(budget);
      b.iter(|| {
        tick.set(tick.get() + 1);
        render_frame(&fonts, black_box(tick.get()));
      });
    });
  }

  set_shape_cache_max_bytes(0);
  group.finish();
}

criterion_group! {
  name = benches;
  config = common::criterion();
  targets = bench_panels
}
criterion_main!(benches);
