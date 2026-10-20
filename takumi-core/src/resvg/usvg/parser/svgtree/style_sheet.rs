//! An SVG document's `<style>` sheets and `style` attributes, matched by takumi's selector engine
//! in place of upstream usvg's `simplecss`.

use std::{cell::RefCell, num::NonZeroUsize, ptr::NonNull};

use cssparser::{
  AtRuleParser, ParseError, Parser, ParserState, QualifiedRuleParser, RuleBodyParser,
  StyleSheetParser,
};
use selectors::{
  Element, OpaqueElement,
  attr::{AttrSelectorOperation, CaseSensitivity, NamespaceConstraint},
  bloom::BloomFilter,
  matching::{
    ElementSelectorFlags, MatchingContext, MatchingForInvalidation, MatchingMode,
    NeedsSelectorFlags, QuirksMode, SelectorCaches, matches_selector,
  },
  parser::{ParseRelative, Selector, SelectorList},
};

use super::parse::XML_NAMESPACE_NS;
use crate::{
  error::StyleSheetParseError,
  matching::lang_matches,
  style::{
    RawDeclaration, RawDeclarationParser,
    selector::{Ident, PseudoClass, PseudoElement, SelectorImpl, TakumiSelectorParser},
  },
};

/// One selector of a rule, pointing at the rule's declarations.
struct Rule {
  selector: Selector<SelectorImpl>,
  block: usize,
}

/// Style rules sorted by specificity, source order breaking ties.
#[derive(Default)]
pub(crate) struct StyleSheet<'a> {
  rules: Vec<Rule>,
  blocks: Vec<Vec<RawDeclaration<'a>>>,
  /// Shared by every node of the one document the sheet styles.
  caches: RefCell<SelectorCaches>,
}

impl<'a> StyleSheet<'a> {
  /// Adds the style rules of `text`, after the rules already parsed.
  pub(crate) fn parse_more(&mut self, text: &'a str) {
    let mut parser = Parser::new(text);

    for (selectors, declarations) in
      StyleSheetParser::new(&mut parser, &mut RuleListParser).filter_map(Result::ok)
    {
      let block = self.blocks.len();

      self.blocks.push(declarations);
      self
        .rules
        .extend(selectors.slice().iter().map(|selector| Rule {
          selector: selector.clone(),
          block,
        }));
    }

    self.rules.sort_by_key(|rule| rule.selector.specificity());
  }

  /// Calls `apply` with each declaration whose rule matches `node`, lowest specificity first.
  pub(crate) fn cascade(&self, node: roxmltree::Node, mut apply: impl FnMut(&RawDeclaration<'a>)) {
    let mut caches = self.caches.borrow_mut();
    let mut context = MatchingContext::new(
      MatchingMode::Normal,
      None,
      &mut caches,
      QuirksMode::NoQuirks,
      NeedsSelectorFlags::No,
      MatchingForInvalidation::No,
    );
    let element = XmlElement(node);

    for rule in &self.rules {
      if matches_selector(&rule.selector, 0, None, &element, &mut context) {
        self.blocks[rule.block].iter().for_each(&mut apply);
      }
    }
  }
}

struct RuleListParser;

impl<'i> QualifiedRuleParser<'i> for RuleListParser {
  type Prelude = SelectorList<SelectorImpl>;
  type QualifiedRule = (SelectorList<SelectorImpl>, Vec<RawDeclaration<'i>>);
  type Error = StyleSheetParseError;

  fn parse_prelude(
    &mut self,
    input: &mut Parser<'i>,
  ) -> Result<Self::Prelude, ParseError<Self::Error>> {
    SelectorList::parse(&TakumiSelectorParser, input, ParseRelative::No)
  }

  fn parse_block(
    &mut self,
    selectors: Self::Prelude,
    _start: &ParserState,
    input: &mut Parser<'i>,
  ) -> Result<Self::QualifiedRule, ParseError<Self::Error>> {
    let declarations = RuleBodyParser::new(input, &mut RawDeclarationParser)
      .filter_map(Result::ok)
      .collect();

    Ok((selectors, declarations))
  }
}

impl<'i> AtRuleParser<'i> for RuleListParser {
  type Prelude = ();
  type AtRule = (SelectorList<SelectorImpl>, Vec<RawDeclaration<'i>>);
  type Error = StyleSheetParseError;
}

/// An XML element, as the `selectors` crate matches it.
#[derive(Clone, Copy, Debug)]
struct XmlElement<'a, 'input>(roxmltree::Node<'a, 'input>);

impl Element for XmlElement<'_, '_> {
  type Impl = SelectorImpl;

  fn opaque(&self) -> OpaqueElement {
    OpaqueElement::from_non_null_ptr(NonNull::without_provenance(
      NonZeroUsize::MIN.saturating_add(self.0.id().get_usize()),
    ))
  }

  fn parent_element(&self) -> Option<Self> {
    self.0.parent_element().map(Self)
  }

  fn parent_node_is_shadow_root(&self) -> bool {
    false
  }

  fn containing_shadow_host(&self) -> Option<Self> {
    None
  }

  fn is_pseudo_element(&self) -> bool {
    false
  }

  fn prev_sibling_element(&self) -> Option<Self> {
    self.0.prev_sibling_element().map(Self)
  }

  fn next_sibling_element(&self) -> Option<Self> {
    self.0.next_sibling_element().map(Self)
  }

  fn first_element_child(&self) -> Option<Self> {
    self.0.first_element_child().map(Self)
  }

  fn is_html_element_in_html_document(&self) -> bool {
    false
  }

  fn has_local_name(&self, local_name: &Ident) -> bool {
    self.0.tag_name().name() == &**local_name
  }

  fn has_namespace(&self, _ns: &Ident) -> bool {
    false
  }

  fn is_same_type(&self, other: &Self) -> bool {
    self.0.tag_name() == other.0.tag_name()
  }

  fn has_id(&self, id: &Ident, _case_sensitivity: CaseSensitivity) -> bool {
    self.0.attribute("id") == Some(&**id)
  }

  fn has_class(&self, name: &Ident, _case_sensitivity: CaseSensitivity) -> bool {
    self
      .0
      .attribute("class")
      .is_some_and(|classes| classes.split_whitespace().any(|class| class == &**name))
  }

  fn imported_part(&self, _name: &Ident) -> Option<Ident> {
    None
  }

  fn is_part(&self, _name: &Ident) -> bool {
    false
  }

  fn is_empty(&self) -> bool {
    !self
      .0
      .children()
      .any(|child| child.is_element() || child.is_text() && child.text() != Some(""))
  }

  fn is_root(&self) -> bool {
    self.0.parent_element().is_none()
  }

  fn has_custom_state(&self, _name: &Ident) -> bool {
    false
  }

  fn attr_matches(
    &self,
    ns: &NamespaceConstraint<&Ident>,
    local_name: &Ident,
    operation: &AttrSelectorOperation<&Ident>,
  ) -> bool {
    let in_null_namespace = match ns {
      NamespaceConstraint::Any => true,
      NamespaceConstraint::Specific(url) => url.is_empty(),
    };

    in_null_namespace
      && self
        .0
        .attribute(&**local_name)
        .is_some_and(|value| operation.eval_str(value))
  }

  fn match_non_ts_pseudo_class(
    &self,
    pseudo_class: &PseudoClass,
    _context: &mut MatchingContext<'_, SelectorImpl>,
  ) -> bool {
    let PseudoClass::Lang(ranges) = pseudo_class else {
      return false;
    };

    self
      .0
      .ancestors()
      .find_map(|node| {
        node
          .attribute((XML_NAMESPACE_NS, "lang"))
          .or_else(|| node.attribute("lang"))
      })
      .is_some_and(|lang| ranges.iter().any(|range| lang_matches(lang, range)))
  }

  fn match_pseudo_element(
    &self,
    _pseudo_element: &PseudoElement,
    _context: &mut MatchingContext<'_, SelectorImpl>,
  ) -> bool {
    false
  }

  fn apply_selector_flags(&self, _flags: ElementSelectorFlags) {}

  fn is_link(&self) -> bool {
    false
  }

  fn is_html_slot_element(&self) -> bool {
    false
  }

  fn add_element_unique_hashes(&self, _filter: &mut BloomFilter) -> bool {
    false
  }
}

#[cfg(test)]
mod tests {
  use super::StyleSheet;

  fn matched(css: &str, svg: &str, id: &str) -> Vec<String> {
    let xml = roxmltree::Document::parse(svg).unwrap();
    let node = xml
      .descendants()
      .find(|node| node.attribute("id") == Some(id))
      .unwrap();
    let mut sheet = StyleSheet::default();
    let mut values = Vec::new();

    sheet.parse_more(css);
    sheet.cascade(node, |declaration| {
      values.push(declaration.value.to_owned())
    });
    values
  }

  #[test]
  fn rules_apply_in_specificity_then_source_order() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="r" class="c"/></svg>"#;

    assert_eq!(
      matched(
        "#r { fill: red } .c { fill: green } rect { fill: blue } rect { fill: gold }",
        svg,
        "r"
      ),
      ["blue", "gold", "green", "red"]
    );
  }

  #[test]
  fn structural_and_negated_selectors_match() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" xml:lang="fr"><g><rect id="a"/><rect id="b"/><circle id="c"/></g></svg>"#;
    let css = "rect:nth-child(2) { fill: second } g > :not(rect) { fill: not-rect } :lang(fr) circle { fill: french } rect:first-child { fill: first }";

    assert_eq!(matched(css, svg, "a"), ["first"]);
    assert_eq!(matched(css, svg, "b"), ["second"]);
    assert_eq!(matched(css, svg, "c"), ["not-rect", "french"]);
  }

  #[test]
  fn a_comment_leaves_an_element_empty() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><g id="comment"><!-- note --></g><g id="text">x</g></svg>"#;

    assert_eq!(
      matched("g:empty { fill: empty }", svg, "comment"),
      ["empty"]
    );
    assert!(matched("g:empty { fill: empty }", svg, "text").is_empty());
  }
}
