use takumi::prelude::*;

use crate::test_utils::run_fixture_test;

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

/// Outside quirks mode a `<table>` start tag ends the open `<p>`, so the table is not inside it.
#[test]
fn html_table_ends_paragraph() {
  render(
    r#"<!DOCTYPE html><p style="background-color:#3b82f6">a<table style="background-color:#ef4444"><tr><td>b</table>"#,
    "html_table_ends_paragraph",
  );
}
