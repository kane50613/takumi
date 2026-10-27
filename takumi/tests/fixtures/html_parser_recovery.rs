use takumi::prelude::*;

use takumi::unstable::html::parse_html;

use crate::test_utils::{
  CONTEXT, TEST_IMAGES, create_test_viewport, run_fixture_test, write_goldens,
};

fn render(body: &str, fixture_name: &str) {
  let node = Node::from_html(body, FromHtmlOptions::default()).unwrap();

  run_fixture_test(node, fixture_name);
}

/// References decode as the HTML tokenizer decodes them, including legacy names without a `;`.
#[test]
fn html_character_references() {
  render(
    "<div>&lt;b&gt; &amp; &copy;&#169;&#xA9; &apos;</div><div>&amp &copy &unknown; &#x110000; &Tab;|&nbspx</div>",
    "html_character_references",
  );
}

/// A `<li>` start tag ends the `<p>` open in the item before it, so the items stay siblings.
#[test]
fn html_implied_paragraph_end() {
  render(
    "<ol><li><p>first<li><p>second<li>third</ol>",
    "html_implied_paragraph_end",
  );
}

/// `<style>` elements style the document, and one whose `media` does not match is skipped.
#[test]
fn html_style_elements() {
  let parsed = parse_html(
    r#"<style>div { width: 200px; height: 100px; background-color: rgb(59, 130, 246) }</style><style media="(max-width: 100px)">div { background-color: rgb(239, 68, 68) }</style><div></div>"#,
    FromHtmlOptions::default(),
  )
  .unwrap();

  write_goldens(
    RenderOptions::builder()
      .viewport(create_test_viewport())
      .node(parsed.node)
      .fonts(&CONTEXT)
      .images(TEST_IMAGES.clone())
      .stylesheet(StyleSheet::parse(&parsed.css.join("\n")).unwrap().into())
      .build(),
    "html_style_elements",
  )
  .unwrap();
}

/// Outside quirks mode a `<table>` start tag ends the open `<p>`, so the table is not inside it.
#[test]
fn html_table_ends_paragraph() {
  render(
    r#"<!DOCTYPE html><p style="background-color:#3b82f6">a<table style="background-color:#ef4444"><tr><td>b</table>"#,
    "html_table_ends_paragraph",
  );
}
