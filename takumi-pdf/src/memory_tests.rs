//! A render's live heap bytes stay flat across the pages of each pass over a long document.

use std::{
  alloc::{GlobalAlloc, Layout, System},
  cell::{Cell, RefCell},
};

use takumi_html::{FromHtmlOptions, from_html};

use crate::{PageMargins, PageOptions, PdfOptions, render_settling, test_support::fonts};

thread_local! {
  static LIVE_BYTES: Cell<isize> = const { Cell::new(0) };
}

fn live_bytes() -> isize {
  LIVE_BYTES.with(Cell::get)
}

fn add_live_bytes(delta: isize) {
  LIVE_BYTES.with(|live| live.set(live.get() + delta));
}

/// Counts the bytes the current thread holds, so tests running beside this one do not move it.
struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    add_live_bytes(layout.size() as isize);
    unsafe { System.alloc(layout) }
  }

  unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
    add_live_bytes(-(layout.size() as isize));
    unsafe { System.dealloc(ptr, layout) }
  }

  unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
    add_live_bytes(layout.size() as isize);
    unsafe { System.alloc_zeroed(layout) }
  }

  unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
    add_live_bytes(new_size as isize - layout.size() as isize);
    unsafe { System.realloc(ptr, layout, new_size) }
  }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn sections(count: usize) -> String {
  let sections: String = (0..count)
    .map(|index| {
      format!(
        "<section><h2>Section {index}</h2><p>Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.</p></section>"
      )
    })
    .collect();

  format!(r#"<div style="font-size: 12px">{sections}</div>"#)
}

/// Live bytes after the early and the late page of each layout pass over the document.
fn live_bytes_per_pass(source: &str, early: usize, late: usize) -> Vec<[isize; 2]> {
  let fonts = fonts();
  let options = PdfOptions::builder()
    .node(from_html(source, FromHtmlOptions::default()).expect("parse the document"))
    .page(PageOptions {
      width: 320.0,
      height: 240.0,
      margin: PageMargins::uniform(24.0),
    })
    .header(
      from_html(
        r#"<div>Page <span class="pageNumber"></span> of <span class="totalPages"></span></div>"#,
        FromHtmlOptions::default(),
      )
      .expect("parse the header"),
    )
    .fonts(&fonts)
    .build();
  let passes = RefCell::new(Vec::new());

  render_settling(options, &|layout| {
    let pages = layout.page_count();
    let mut passes = passes.borrow_mut();

    if pages == 1 {
      passes.push([0, 0]);
    }
    if let Some(pass) = passes.last_mut() {
      if pages == early {
        pass[0] = live_bytes();
      } else if pages == late {
        pass[1] = live_bytes();
      }
    }
    layout.settled_pages()
  })
  .expect("the document renders");
  passes.into_inner()
}

#[test]
fn a_total_pages_document_keeps_live_bytes_flat_through_both_passes() {
  const EARLY_PAGE: usize = 50;
  const LATE_PAGE: usize = 200;

  let passes = live_bytes_per_pass(&sections(700), EARLY_PAGE, LATE_PAGE);

  assert_eq!(passes.len(), 2, "one pass counts, one draws");
  for (pass, [early, late]) in passes.into_iter().enumerate() {
    assert!(
      early > 0 && late > 0,
      "pass {pass} has fewer than {LATE_PAGE} pages"
    );
    assert!(
      late <= early + early / 10,
      "pass {pass} grew from {early} bytes after page {EARLY_PAGE} to {late} after page {LATE_PAGE}"
    );
  }
}
