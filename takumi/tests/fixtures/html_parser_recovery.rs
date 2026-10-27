use takumi::prelude::*;

use crate::test_utils::run_fixture_test;

fn render(body: &str, fixture_name: &str) {
  let node = Node::from_html(body, FromHtmlOptions::default()).unwrap();

  run_fixture_test(node, fixture_name);
}

/// References decode only when they end in `;` and name an HTML4 entity or `&apos;`; the rest stay
/// as written.
#[test]
fn html_character_references() {
  render(
    "<div>&lt;b&gt; &amp; &copy;&#169;&#xA9; &apos;</div><div>&amp &copy &unknown; &#x110000;</div>",
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
