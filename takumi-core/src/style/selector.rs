use std::{
  collections::{HashMap, hash_map::Entry},
  fmt::{self, Write},
  mem::take,
  ops::Deref,
  sync::{
    Arc, LazyLock,
    atomic::{AtomicUsize, Ordering},
  },
};

use cssparser::*;
use precomputed_hash::PrecomputedHash;
use selectors::parser::{
  NonTSPseudoClass, ParseRelative, PseudoElement as PseudoElementTrait,
  SelectorImpl as SelectorImplTrait, SelectorList,
};

use crate::style::ContainerQuery;
#[cfg(feature = "font-face")]
use crate::style::font_face_rule::FontFaceRule;
pub use crate::style::media_query::MediaQueryList;
use crate::style::{
  DeclarationOrigin, IgnoredDeclaration, IgnoredReason, PageName, PropertySyntax,
  page_rule::{PageContext, PageDescriptors, PageKind, PageRule, PageSelector, PageStyles},
};
use crate::viewport::Viewport;
use crate::{
  Xxh3HashSet,
  error::{StyleSheetParseError, StyleSheetParseErrorKind},
  keyframes::parse_keyframe_prelude,
  matching::RuleIndex,
  style::{
    BreakpointOverrides, FromCssStr, KeyframeRule, KeyframesRule, Length, RawDeclaration,
    RawDeclarationParser, StyleDeclaration, StyleDeclarationBlock, ThemeKeyframes, expand_apply,
    is_reserved_custom_ident, supports::parse_supports_condition,
  },
};

/// A registered custom property from an `@property` rule.
#[derive(Debug, Clone, PartialEq)]
pub struct PropertyRule {
  /// Custom property name, including the leading `--`.
  pub name: String,
  /// The values the property takes.
  pub syntax: PropertySyntax,
  /// Whether the property inherits.
  pub inherits: bool,
  /// Declared `initial-value`, if any.
  pub initial_value: Option<String>,
  /// Media queries this rule is gated behind.
  pub media_queries: Vec<MediaQueryList>,
}

/// A name in a cascade layer path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum LayerName {
  /// An explicitly named layer.
  Named(String),
  /// An anonymous layer, which no other `@layer` names, as Blink's anonymous `CascadeLayer`s are
  /// all distinct.
  Anonymous(usize),
}

impl LayerName {
  /// A new anonymous layer.
  fn anonymous() -> Self {
    static NEXT_ANONYMOUS: AtomicUsize = AtomicUsize::new(0);

    Self::Anonymous(NEXT_ANONYMOUS.fetch_add(1, Ordering::Relaxed))
  }
}

pub(crate) type LayerPath = Vec<LayerName>;

/// Approximate: a cascade layer path nests at most this many layers deep, and an `@layer` rule
/// naming a deeper layer is invalid, where Blink has no limit.
const MAX_LAYER_DEPTH: usize = 64;

/// The cascade layer tree, each layer's direct sublayers in the order they first appear, as Blink's
/// `CascadeLayer` holds them. Layer 0 is the implicit outer layer.
struct LayerTree {
  sublayers: Vec<Vec<usize>>,
  /// The sublayer of a layer by its name.
  by_name: HashMap<(usize, LayerName), usize>,
}

impl Default for LayerTree {
  fn default() -> Self {
    Self {
      sublayers: vec![Vec::new()],
      by_name: HashMap::new(),
    }
  }
}

impl LayerTree {
  /// Adds the layer at `path` and each layer above it that is not yet in the tree, and returns it.
  fn insert(&mut self, path: &[LayerName]) -> usize {
    let mut layer = 0;

    for name in path {
      layer = match self.by_name.entry((layer, name.clone())) {
        Entry::Occupied(entry) => *entry.get(),
        Entry::Vacant(entry) => {
          let sublayer = self.sublayers.len();

          entry.insert(sublayer);
          self.sublayers[layer].push(sublayer);
          self.sublayers.push(Vec::new());
          sublayer
        }
      };
    }
    layer
  }

  /// Each layer's order, after its sublayers', as Blink's `ComputeLayerOrder` numbers the tree.
  /// The outer layer's is the count of the others.
  fn orders(&self) -> Vec<usize> {
    let mut orders = vec![0; self.sublayers.len()];
    let mut next = 0;
    let mut stack = vec![(0, 0)];

    while let Some(top) = stack.last_mut() {
      let (layer, child) = *top;

      match self.sublayers[layer].get(child) {
        Some(&sublayer) => {
          top.1 += 1;
          stack.push((sublayer, 0));
        }
        None => {
          orders[layer] = next;
          next += 1;
          stack.pop();
        }
      }
    }
    orders
  }
}

/// An ASCII-case-insensitive CSS identifier.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Ident(String);

impl Deref for Ident {
  type Target = str;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl AsRef<str> for Ident {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

impl PartialEq<&str> for Ident {
  fn eq(&self, other: &&str) -> bool {
    self.0 == *other
  }
}

impl PartialEq<Ident> for &str {
  fn eq(&self, other: &Ident) -> bool {
    self == &other.0
  }
}

impl From<&str> for Ident {
  fn from(s: &str) -> Self {
    Self(s.to_owned())
  }
}

impl ToCss for Ident {
  fn to_css<W>(&self, dest: &mut W) -> fmt::Result
  where
    W: Write,
  {
    serialize_identifier(&self.0, dest)
  }
}

impl PrecomputedHash for Ident {
  fn precomputed_hash(&self) -> u32 {
    let mut hash = 0x811c9dc5u32;
    for byte in self.0.as_bytes() {
      hash ^= u32::from(byte.to_ascii_lowercase());
      hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
  }
}

/// Selector type bindings for the `selectors` crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelectorImpl;

// Pseudo-classes parse so a selector list keeps its other selectors, and only the static ones,
// such as `:lang()`, `:dir()`, and the form control states, match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PseudoClass {
  /// `:lang(en)`, by the one identifier stable Blink takes, as its `CSSSelectorParser` parses it
  /// without `CSSLangExtendedRanges`.
  Lang(Ident),
  /// `:dir(ltr)` or `:dir(rtl)`, by the keyword it names; any other keyword matches nothing.
  Dir(Ident),
  /// A form control state, such as `:disabled`.
  FormState(FormState),
  /// Any other pseudo-class, parsed but never matched.
  Other(Ident),
}

impl ToCss for PseudoClass {
  fn to_css<W>(&self, dest: &mut W) -> fmt::Result
  where
    W: Write,
  {
    match self {
      Self::Lang(range) => {
        dest.write_str(":lang(")?;
        range.to_css(dest)?;
        dest.write_char(')')
      }
      Self::Dir(direction) => {
        dest.write_str(":dir(")?;
        direction.to_css(dest)?;
        dest.write_char(')')
      }
      Self::FormState(state) => {
        dest.write_char(':')?;
        dest.write_str(state.name())
      }
      Self::Other(name) => {
        dest.write_char(':')?;
        name.to_css(dest)
      }
    }
  }
}

/// A pseudo-class HTML defines on form control state.
/// <https://html.spec.whatwg.org/multipage/semantics-other.html#pseudo-classes>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FormState {
  Disabled,
  Enabled,
  Checked,
  Indeterminate,
  PlaceholderShown,
  ReadOnly,
  ReadWrite,
  Required,
  Optional,
}

impl FormState {
  const NAMES: [(&'static str, Self); 9] = [
    ("disabled", Self::Disabled),
    ("enabled", Self::Enabled),
    ("checked", Self::Checked),
    ("indeterminate", Self::Indeterminate),
    ("placeholder-shown", Self::PlaceholderShown),
    ("read-only", Self::ReadOnly),
    ("read-write", Self::ReadWrite),
    ("required", Self::Required),
    ("optional", Self::Optional),
  ];

  fn from_name(name: &str) -> Option<Self> {
    Self::NAMES
      .iter()
      .find(|(state_name, _)| state_name.eq_ignore_ascii_case(name))
      .map(|&(_, state)| state)
  }

  fn name(self) -> &'static str {
    Self::NAMES
      .iter()
      .find(|&&(_, state)| state == self)
      .map_or("", |&(name, _)| name)
  }
}

impl NonTSPseudoClass for PseudoClass {
  fn is_active_or_hover(&self) -> bool {
    false
  }
  fn is_user_action_state(&self) -> bool {
    false
  }
}

/// A pseudo-element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PseudoElement {
  /// `::before`.
  Before,
  /// `::after`.
  After,
  /// Any other pseudo-element, kept but never matched.
  Other(Ident),
}

impl PseudoElement {
  fn from_name(name: &str) -> Self {
    if name.eq_ignore_ascii_case("before") {
      Self::Before
    } else if name.eq_ignore_ascii_case("after") {
      Self::After
    } else {
      Self::Other(Ident::from(name))
    }
  }

  fn as_str(&self) -> &str {
    match self {
      Self::Before => "before",
      Self::After => "after",
      Self::Other(name) => name,
    }
  }
}

impl ToCss for PseudoElement {
  fn to_css<W>(&self, dest: &mut W) -> fmt::Result
  where
    W: Write,
  {
    dest.write_str("::")?;
    serialize_identifier(self.as_str(), dest)
  }
}

impl PseudoElementTrait for PseudoElement {}

impl SelectorImplTrait for SelectorImpl {
  type ExtraMatchingData<'a> = ();
  type AttrValue = Ident;
  type Identifier = Ident;
  type LocalName = Ident;
  type NamespaceUrl = Ident;
  type NamespacePrefix = Ident;
  type BorrowedNamespaceUrl = Ident;
  type BorrowedLocalName = Ident;
  type NonTSPseudoClass = PseudoClass;
  type PseudoElement = PseudoElement;
}

pub(crate) struct TakumiSelectorParser;

impl<'i> selectors::Parser<'i> for TakumiSelectorParser {
  type Impl = SelectorImpl;
  type Error = StyleSheetParseError;

  fn parse_parent_selector(&self) -> bool {
    true
  }

  fn parse_host(&self) -> bool {
    true
  }

  fn parse_is_and_where(&self) -> bool {
    true
  }

  fn parse_has(&self) -> bool {
    true
  }

  fn parse_non_ts_pseudo_class(
    &self,
    name: &str,
  ) -> Result<<Self::Impl as SelectorImplTrait>::NonTSPseudoClass, ParseError<Self::Error>> {
    Ok(FormState::from_name(name).map_or_else(
      || PseudoClass::Other(Ident::from(name)),
      PseudoClass::FormState,
    ))
  }

  fn parse_non_ts_functional_pseudo_class(
    &self,
    name: &str,
    parser: &mut Parser<'i>,
    _after_part: bool,
  ) -> Result<<Self::Impl as SelectorImplTrait>::NonTSPseudoClass, ParseError<Self::Error>> {
    if name.eq_ignore_ascii_case("lang") {
      let range = parser.expect_ident()?;

      return Ok(PseudoClass::Lang(Ident::from(range.as_ref())));
    }

    if name.eq_ignore_ascii_case("dir") {
      let direction = parser.expect_ident()?;

      return Ok(PseudoClass::Dir(Ident::from(
        &*direction.to_ascii_lowercase(),
      )));
    }

    while parser.next_including_whitespace_and_comments().is_ok() {}
    Ok(PseudoClass::Other(Ident::from(name)))
  }

  fn parse_pseudo_element(
    &self,
    name: &str,
  ) -> Result<<Self::Impl as SelectorImplTrait>::PseudoElement, ParseError<Self::Error>> {
    Ok(PseudoElement::from_name(name))
  }

  fn parse_functional_pseudo_element(
    &self,
    name: &str,
    arguments: &mut Parser<'i>,
  ) -> Result<<Self::Impl as SelectorImplTrait>::PseudoElement, ParseError<Self::Error>> {
    while arguments.next_including_whitespace_and_comments().is_ok() {}
    Ok(PseudoElement::Other(Ident::from(name)))
  }
}

#[derive(Debug, Clone, Default)]
struct StyleSheetFragment {
  rules: Vec<CssRule>,
  keyframes: Vec<KeyframesRule>,
  /// The theme `@keyframes` its `@apply` rules read, which rank below every `@keyframes` rule.
  theme_keyframes: ThemeKeyframes,
  property_rules: Vec<PropertyRule>,
  #[cfg(feature = "font-face")]
  font_faces: Vec<FontFaceRule>,
  page_rules: Vec<PageRule>,
  declared_layers: Vec<LayerPath>,
  preflight: bool,
  /// The declarations and at-rules parsing dropped.
  ignored: Vec<IgnoredDeclaration>,
}

impl StyleSheetFragment {
  /// A fragment that holds only the at-rule parsing dropped.
  fn ignoring(at_rule: IgnoredDeclaration) -> Self {
    Self {
      ignored: vec![at_rule],
      ..Self::default()
    }
  }

  fn extend(&mut self, other: Self) {
    self.rules.extend(other.rules);
    self.keyframes.extend(other.keyframes);
    self.theme_keyframes.extend(&other.theme_keyframes);
    self.property_rules.extend(other.property_rules);
    #[cfg(feature = "font-face")]
    self.font_faces.extend(other.font_faces);
    self.page_rules.extend(other.page_rules);
    self.declared_layers.extend(other.declared_layers);
    self.preflight |= other.preflight;
    self.ignored.extend(other.ignored);
  }
}

#[derive(Debug)]
enum StyleRuleBodyItem {
  Declarations(Box<StyleDeclarationBlock>),
  Rules(StyleSheetFragment),
}

/// Declarations gathered for one rule, split at the importance boundary.
#[derive(Default)]
struct RuleDeclarations {
  normal: StyleDeclarationBlock,
  important: StyleDeclarationBlock,
}

impl RuleDeclarations {
  /// Adds one parsed declaration, whose block is important as a whole or not at all.
  fn push(&mut self, block: StyleDeclarationBlock) {
    if block.importance.is_empty() {
      self.normal.append(block);
    } else {
      self.important.append(block);
    }
  }

  fn is_empty(&self) -> bool {
    self.normal.declarations.is_empty() && self.important.declarations.is_empty()
  }

  /// Moves out the declarations parsing dropped from both blocks.
  fn take_ignored(&mut self) -> impl Iterator<Item = IgnoredDeclaration> {
    take(&mut self.normal.ignored)
      .into_iter()
      .chain(take(&mut self.important.ignored))
  }
}

/// What a nested rule inherits from the rules around it: the media queries, layer, scope, and
/// container queries gating it, and whether invalid items are dropped.
#[derive(Clone, Copy)]
struct NestingContext<'a> {
  media_queries: &'a [MediaQueryList],
  layer: Option<&'a LayerPath>,
  /// The scope of an `@scope` rule nested in a style rule around it.
  scope: Option<&'a Arc<Scope>>,
  /// The queries of `@container` rules nested in style rules around it.
  container_queries: &'a [Arc<ContainerQuery>],
  /// In the direct body of a nested `@scope`, where a selector is relative to the scope's root.
  scope_body: bool,
  /// The owner the sheet's [`STYLE_OWNER_RULE`] names.
  style_owner: Option<u32>,
  lossy: bool,
}

/// The items a rule or declaration list yields, with the invalid ones dropped
/// when `lossy`, each dropped declaration added to `ignored`, and their errors
/// kept otherwise.
fn recover<'i, T>(
  items: impl Iterator<Item = Result<T, (ParseError<StyleSheetParseError>, &'i str, SourceLocation)>>,
  lossy: bool,
  ignored: &mut Vec<IgnoredDeclaration>,
) -> impl Iterator<Item = Result<T, ParseError<StyleSheetParseError>>> {
  items.filter_map(move |item| match item {
    Ok(item) => Some(Ok(item)),
    Err((_, source, location)) if lossy => {
      log_ignored(source, location);
      ignored.extend(IgnoredDeclaration::invalid_value(
        source,
        DeclarationOrigin::StyleSheet,
      ));
      None
    }
    Err((error, ..)) => Some(Err(error)),
  })
}

/// Logs the CSS a lossy parse dropped, `source` cut to its first line.
pub(crate) fn log_ignored(source: &str, location: SourceLocation) {
  log::debug!(
    target: "takumi::style",
    "ignored `{}` at line {}, column {} of its style sheet or `style` attribute",
    source.trim().lines().next().unwrap_or_default(),
    location.line + 1,
    location.column
  );
}

macro_rules! impl_parser_traits {
  ($parser:ty, $item:ty) => {
    impl<'i> QualifiedRuleParser<'i> for $parser {
      type Prelude = ();
      type QualifiedRule = $item;
      type Error = StyleSheetParseError;
    }

    impl<'i> AtRuleParser<'i> for $parser {
      type Prelude = ();
      type AtRule = $item;
      type Error = StyleSheetParseError;
    }

    impl<'i> RuleBodyItemParser<'i, $item, StyleSheetParseError> for $parser {
      fn parse_qualified(&self) -> bool {
        false
      }
      fn parse_declarations(&self) -> bool {
        true
      }
    }
  };
}

/// Parses one declaration, recording a property takumi does not support as written in `origin`.
pub(crate) struct StyleDeclarationParser {
  pub(crate) origin: DeclarationOrigin,
}

impl<'i> DeclarationParser<'i> for StyleDeclarationParser {
  type Declaration = StyleDeclarationBlock;
  type Error = StyleSheetParseError;

  fn parse_value(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i>,
    _state: &ParserState,
  ) -> Result<Self::Declaration, ParseError<Self::Error>> {
    let mut declarations =
      StyleDeclarationBlock::parse_declaration_value(&name, input, self.origin)
        .map_err(ParseError::into)?;
    if input.try_parse(parse_important).is_ok() {
      declarations.mark_important();
    }
    Ok(declarations)
  }
}

impl_parser_traits!(StyleDeclarationParser, StyleDeclarationBlock);

struct NestedStyleRuleParser<'a> {
  parent_selectors: SelectorList<SelectorImpl>,
  context: NestingContext<'a>,
  /// Whether the body takes declarations as well as rules.
  declarations: bool,
}

/// One declaration of a rule body, which a style sheet wrote.
fn parse_body_declaration<'i>(
  name: CowRcStr<'i>,
  input: &mut Parser<'i>,
  state: &ParserState,
) -> Result<StyleRuleBodyItem, ParseError<StyleSheetParseError>> {
  let mut parser = StyleDeclarationParser {
    origin: DeclarationOrigin::StyleSheet,
  };
  parser
    .parse_value(name, input, state)
    .map(Box::new)
    .map(StyleRuleBodyItem::Declarations)
}

impl<'i> DeclarationParser<'i> for NestedStyleRuleParser<'_> {
  type Declaration = StyleRuleBodyItem;
  type Error = StyleSheetParseError;

  fn parse_value(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i>,
    state: &ParserState,
  ) -> Result<Self::Declaration, ParseError<Self::Error>> {
    parse_body_declaration(name, input, state)
  }
}

impl<'i> QualifiedRuleParser<'i> for NestedStyleRuleParser<'_> {
  type Prelude = SelectorList<SelectorImpl>;
  type QualifiedRule = StyleRuleBodyItem;
  type Error = StyleSheetParseError;

  fn parse_prelude(
    &mut self,
    input: &mut Parser<'i>,
  ) -> Result<Self::Prelude, ParseError<Self::Error>> {
    let relative = if self.context.scope_body {
      ParseRelative::ForScope
    } else {
      ParseRelative::ForNesting
    };

    SelectorList::parse(&TakumiSelectorParser, input, relative)
  }

  fn parse_block(
    &mut self,
    nested_selectors: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i>,
  ) -> Result<Self::QualifiedRule, ParseError<Self::Error>> {
    let selectors = nested_selectors.replace_parent_selector(&self.parent_selectors);

    NestingContext {
      scope_body: false,
      ..self.context
    }
    .parse_style_rule_block(selectors, input)
    .map(StyleRuleBodyItem::Rules)
  }
}

impl<'i> AtRuleParser<'i> for NestedStyleRuleParser<'_> {
  type Prelude = AtRulePrelude;
  type AtRule = StyleRuleBodyItem;
  type Error = StyleSheetParseError;

  fn parse_prelude(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i>,
  ) -> Result<Self::Prelude, ParseError<Self::Error>> {
    parse_at_rule_prelude(name, input, ParseRelative::ForNesting)
  }

  fn parse_block(
    &mut self,
    prelude: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i>,
  ) -> Result<Self::AtRule, ParseError<Self::Error>> {
    self
      .context
      .parse_nested_at_rule_block(&self.parent_selectors, prelude, input)
      .map(StyleRuleBodyItem::Rules)
  }

  fn rule_without_block(
    &mut self,
    prelude: Self::Prelude,
    _start: &ParserState,
  ) -> Result<Self::AtRule, ()> {
    match prelude {
      // A rule on the parent's selectors, so declarations around `@apply` keep their order.
      AtRulePrelude::Apply(block, theme_keyframes) => {
        let (normal, important) = block.split_importance();

        Ok(StyleRuleBodyItem::Rules(StyleSheetFragment {
          rules: vec![self.context.rule(
            self.parent_selectors.clone(),
            RuleDeclarations { normal, important },
          )],
          theme_keyframes,
          ..StyleSheetFragment::default()
        }))
      }
      AtRulePrelude::Unknown(at_rule) => Ok(StyleRuleBodyItem::Rules(
        StyleSheetFragment::ignoring(*at_rule),
      )),
      _ => Err(()),
    }
  }
}

impl<'i> RuleBodyItemParser<'i, StyleRuleBodyItem, StyleSheetParseError>
  for NestedStyleRuleParser<'_>
{
  fn parse_qualified(&self) -> bool {
    true
  }

  fn parse_declarations(&self) -> bool {
    self.declarations
  }
}

struct KeyframeDeclarationParser;

impl<'i> DeclarationParser<'i> for KeyframeDeclarationParser {
  type Declaration = StyleDeclarationBlock;
  type Error = StyleSheetParseError;

  fn parse_value(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i>,
    _state: &ParserState,
  ) -> Result<Self::Declaration, ParseError<Self::Error>> {
    // A keyframe declaration takes no `!important`, so one that has it is left unread and invalid.
    // <https://drafts.csswg.org/css-animations-1/#keyframes>
    StyleDeclarationBlock::parse(&name, input, DeclarationOrigin::StyleSheet)
      .map_err(ParseError::into)
  }
}

impl_parser_traits!(KeyframeDeclarationParser, StyleDeclarationBlock);

struct KeyframeRuleParser {
  lossy: bool,
}

impl<'i> QualifiedRuleParser<'i> for KeyframeRuleParser {
  type Prelude = Vec<f32>;
  type QualifiedRule = KeyframeRule;
  type Error = StyleSheetParseError;

  fn parse_prelude(
    &mut self,
    input: &mut Parser<'i>,
  ) -> Result<Self::Prelude, ParseError<Self::Error>> {
    parse_keyframe_prelude(input)
  }

  fn parse_block(
    &mut self,
    offsets: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i>,
  ) -> Result<Self::QualifiedRule, ParseError<Self::Error>> {
    let mut declaration_parser = KeyframeDeclarationParser;
    let mut declarations = StyleDeclarationBlock::default();
    let mut ignored = Vec::new();

    for block in recover(
      RuleBodyParser::new(input, &mut declaration_parser),
      self.lossy,
      &mut ignored,
    ) {
      declarations.append(block?);
    }
    declarations.ignored.extend(ignored);

    Ok(KeyframeRule {
      offsets,
      declarations,
    })
  }
}

impl<'i> AtRuleParser<'i> for KeyframeRuleParser {
  type Prelude = ();
  type AtRule = KeyframeRule;
  type Error = StyleSheetParseError;
}

struct RuleParser {
  current_layer: Option<LayerPath>,
  /// The owner the sheet's [`STYLE_OWNER_RULE`] names.
  style_owner: Option<u32>,
  lossy: bool,
  /// Inside `@scope`, where a selector without `:scope` or `&` is relative to the scope's root.
  scoped: bool,
  /// False inside `@media`, `@layer` and `@supports` blocks, where `@import`
  /// is invalid and its preflight flag must not escape the gate.
  top_level: bool,
}

impl RuleParser {
  /// The parser for a block gated by an at-rule, filling `current_layer`.
  fn nested(&self, current_layer: Option<LayerPath>) -> Self {
    Self {
      current_layer,
      style_owner: self.style_owner,
      lossy: self.lossy,
      scoped: self.scoped,
      top_level: false,
    }
  }

  /// What a rule at this level hands the rules nested in it.
  fn context(&self) -> NestingContext<'_> {
    NestingContext {
      media_queries: &[],
      layer: self.current_layer.as_ref(),
      scope: None,
      container_queries: &[],
      scope_body: false,
      style_owner: self.style_owner,
      lossy: self.lossy,
    }
  }

  /// The layers `layer_names` declare, as paths under the current layer.
  fn declared_layers(
    &self,
    layer_names: &[LayerPath],
  ) -> Result<Vec<LayerPath>, ParseError<StyleSheetParseError>> {
    layer_names
      .iter()
      .map(|layer_name| extend_layer_name(self.current_layer.as_ref(), layer_name))
      .collect()
  }

  fn parse_fragment<'i>(
    &mut self,
    input: &mut Parser<'i>,
  ) -> Result<StyleSheetFragment, ParseError<StyleSheetParseError>> {
    let lossy = self.lossy;
    let mut fragment = StyleSheetFragment::default();
    let mut ignored = Vec::new();

    for nested in recover(StyleSheetParser::new(input, self), lossy, &mut ignored) {
      fragment.extend(nested?);
    }
    fragment.ignored.extend(ignored);

    Ok(fragment)
  }
}

#[derive(Debug, Clone)]
enum AtRulePrelude {
  Keyframes(String),
  Layer(Vec<LayerPath>),
  Media(MediaQueryList),
  Property(String),
  #[cfg(feature = "font-face")]
  FontFace,
  Page(PageSelector),
  Supports(bool),
  /// Tailwind's `@theme`, read as the `:root` rule it compiles to, whatever its modifiers.
  Theme,
  /// A Tailwind entry point such as `@import "tailwindcss"`, which only turns Preflight on.
  TailwindImport {
    preflight: bool,
  },
  /// `@apply`, already expanded into the declarations its utilities stand for and the theme
  /// `@keyframes` they read.
  Apply(Box<StyleDeclarationBlock>, ThemeKeyframes),
  /// `@scope`, its root and limit selectors.
  Scope(Scope),
  /// `@container`, its query.
  Container(ContainerQuery),
  /// The owner [`STYLE_OWNER_RULE`] names, none for `none`.
  StyleOwner(Option<u32>),
  /// An at-rule takumi does not implement, such as `@counter-style`, dropped
  /// without failing the sheet: <https://drafts.csswg.org/css-syntax-3/#consume-at-rule>.
  Unknown(Box<IgnoredDeclaration>),
}

/// The scope an `@scope` rule gives the style rules in it: the elements its `start` selectors
/// match, or without them its `<style>` element's parent or the tree's root, down to but not into the elements its `end` selectors
/// match. <https://drafts.csswg.org/css-cascade-6/#scoped-styles>
#[derive(Debug, Clone)]
pub(crate) struct Scope {
  pub(crate) start: Option<SelectorList<SelectorImpl>>,
  pub(crate) end: Option<SelectorList<SelectorImpl>>,
  /// The owner of the `<style>` element holding a rule without `start`, whose element is its root.
  pub(crate) owner: Option<u32>,
  /// The scope of an `@scope` rule this one is in, which its roots must be in.
  pub(crate) parent: Option<Arc<Scope>>,
}

impl Scope {
  /// This scope inside `outer`, at the end of its chain of enclosing scopes.
  fn within(&self, outer: &Arc<Scope>) -> Arc<Scope> {
    Arc::new(Self {
      parent: Some(match &self.parent {
        Some(parent) => parent.within(outer),
        None => Arc::clone(outer),
      }),
      ..self.clone()
    })
  }
}

/// A style rule with its selectors and declaration blocks. Opaque: its
/// `selectors` crate innards stay out of takumi's public API.
#[derive(Debug, Clone)]
pub(crate) struct CssRule {
  /// Selectors this rule applies to.
  pub(crate) selectors: SelectorList<SelectorImpl>,
  /// Declarations without `!important`.
  pub(crate) normal_declarations: StyleDeclarationBlock,
  /// Declarations marked `!important`.
  pub(crate) important_declarations: StyleDeclarationBlock,
  /// Media queries gating this rule.
  pub(crate) media_queries: Vec<MediaQueryList>,
  /// Cascade layer this rule belongs to.
  pub(crate) layer: Option<LayerPath>,
  /// Resolved ordinal of the rule's layer.
  pub(crate) layer_order: Option<usize>,
  /// The scope of the `@scope` rule it is in.
  pub(crate) scope: Option<Arc<Scope>>,
  /// The queries of the `@container` rules it is in, which must all hold.
  pub(crate) container_queries: Vec<Arc<ContainerQuery>>,
}

fn parse_property_rule<'i>(
  property_name: String,
  input: &mut Parser<'i>,
) -> Result<PropertyRule, ParseError<StyleSheetParseError>> {
  let mut parser = RawDeclarationParser;
  let mut syntax = None;
  let mut inherits = None;
  let mut initial_value = None;
  let mut invalid_inherits = false;

  for descriptor in RuleBodyParser::new(input, &mut parser) {
    let RawDeclaration {
      name,
      value,
      important,
    } = descriptor.map_err(|(error, ..)| error)?;

    // Blink parses descriptors without the `!important` annotation, so one that has it is invalid.
    if important {
      continue;
    }

    match_ignore_ascii_case! { &name,
      "syntax" => syntax = Some(value.to_owned()),
      "inherits" => match_ignore_ascii_case! { &value,
        "true" => inherits = Some(true),
        "false" => inherits = Some(false),
        _ => invalid_inherits = true,
      },
      "initial-value" => initial_value = Some(value.to_owned()),
      _ => {},
    }
  }

  if invalid_inherits {
    return Err(ParseError::custom(
      StyleSheetParseErrorKind::PropertyInheritsMustBeBoolean,
    ));
  }

  let Some(syntax) = syntax else {
    return Err(ParseError::custom(
      StyleSheetParseErrorKind::MissingPropertySyntax,
    ));
  };
  let Some(inherits) = inherits else {
    return Err(ParseError::custom(
      StyleSheetParseErrorKind::MissingPropertyInherits,
    ));
  };

  // The descriptor is one CSS string, which the syntax is read from once decoded.
  let mut syntax_input = Parser::new(&syntax);
  let decoded = syntax_input.expect_string().map(ToString::to_string).ok();
  let syntax = decoded
    .filter(|_| syntax_input.is_exhausted())
    .and_then(|decoded| decoded.parse::<PropertySyntax>().ok())
    .ok_or_else(|| ParseError::custom(StyleSheetParseErrorKind::InvalidPropertySyntax))?;

  // css-properties-values-api-1 2.1: only the universal syntax may leave the initial value out,
  // and any other must take it and compute it without an element.
  let initial_value = match (&syntax, initial_value) {
    (PropertySyntax::Universal, initial_value) => initial_value,
    (_, None) => {
      return Err(ParseError::custom(
        StyleSheetParseErrorKind::MissingPropertyInitialValue,
      ));
    }
    (_, Some(value)) if syntax.is_computationally_independent(&value) => {
      syntax.compute(&value, None)
    }
    _ => {
      return Err(ParseError::custom(
        StyleSheetParseErrorKind::InvalidPropertyInitialValue,
      ));
    }
  };

  Ok(PropertyRule {
    name: property_name,
    syntax,
    inherits,
    initial_value,
    media_queries: Vec::new(),
  })
}

/// Parses a `@theme` body into `:root` declarations and the `@keyframes` it carries.
struct ThemeBlockParser<'a> {
  context: NestingContext<'a>,
}

enum ThemeBodyItem {
  Declarations(Box<StyleDeclarationBlock>),
  Keyframes(StyleSheetFragment),
}

impl<'i> DeclarationParser<'i> for ThemeBlockParser<'_> {
  type Declaration = ThemeBodyItem;
  type Error = StyleSheetParseError;

  fn parse_value(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i>,
    state: &ParserState,
  ) -> Result<Self::Declaration, ParseError<Self::Error>> {
    let mut parser = StyleDeclarationParser {
      origin: DeclarationOrigin::StyleSheet,
    };
    parser
      .parse_value(name, input, state)
      .map(Box::new)
      .map(ThemeBodyItem::Declarations)
  }
}

impl<'i> AtRuleParser<'i> for ThemeBlockParser<'_> {
  type Prelude = AtRulePrelude;
  type AtRule = ThemeBodyItem;
  type Error = StyleSheetParseError;

  fn parse_prelude(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i>,
  ) -> Result<Self::Prelude, ParseError<Self::Error>> {
    parse_at_rule_prelude(name, input, ParseRelative::No)
  }

  fn parse_block(
    &mut self,
    prelude: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i>,
  ) -> Result<Self::AtRule, ParseError<Self::Error>> {
    match prelude {
      AtRulePrelude::Keyframes(name) => self
        .context
        .parse_keyframes_block(name, input)
        .map(ThemeBodyItem::Keyframes),
      _ => Err(ParseError::custom(
        StyleSheetParseErrorKind::UnsupportedNestedAtRule,
      )),
    }
  }
}

impl<'i> QualifiedRuleParser<'i> for ThemeBlockParser<'_> {
  type Prelude = ();
  type QualifiedRule = ThemeBodyItem;
  type Error = StyleSheetParseError;

  fn parse_prelude(
    &mut self,
    _input: &mut Parser<'i>,
  ) -> Result<Self::Prelude, ParseError<Self::Error>> {
    Err(ParseError::custom(
      StyleSheetParseErrorKind::UnsupportedNestedAtRule,
    ))
  }
}

impl<'i> RuleBodyItemParser<'i, ThemeBodyItem, StyleSheetParseError> for ThemeBlockParser<'_> {
  fn parse_qualified(&self) -> bool {
    false
  }

  fn parse_declarations(&self) -> bool {
    true
  }
}

impl NestingContext<'_> {
  /// A rule for `selectors` gated the way this context gates it.
  fn rule(self, selectors: SelectorList<SelectorImpl>, declarations: RuleDeclarations) -> CssRule {
    CssRule {
      selectors,
      normal_declarations: declarations.normal,
      important_declarations: declarations.important,
      media_queries: self.media_queries.to_vec(),
      layer: self.layer.cloned(),
      layer_order: None,
      scope: self.scope.cloned(),
      container_queries: self.container_queries.to_vec(),
    }
  }

  fn parse_theme_block<'i>(
    self,
    input: &mut Parser<'i>,
  ) -> Result<StyleSheetFragment, ParseError<StyleSheetParseError>> {
    let selectors = SelectorList::parse(
      &TakumiSelectorParser,
      &mut Parser::new(":root"),
      ParseRelative::No,
    )
    .map_err(|_| ParseError::custom(StyleSheetParseErrorKind::UnsupportedNestedAtRule))?;

    let mut declarations = RuleDeclarations::default();
    let mut fragment = StyleSheetFragment::default();
    let mut parser = ThemeBlockParser { context: self };
    let mut ignored = Vec::new();

    for item in recover(
      RuleBodyParser::new(input, &mut parser),
      self.lossy,
      &mut ignored,
    ) {
      match item? {
        ThemeBodyItem::Declarations(block) => declarations.push(*block),
        ThemeBodyItem::Keyframes(keyframes) => fragment.extend(keyframes),
      }
    }
    fragment.ignored.extend(declarations.take_ignored());
    fragment.ignored.extend(ignored);

    if !declarations.is_empty() {
      fragment.rules.push(self.rule(selectors, declarations));
    }

    Ok(fragment)
  }

  fn parse_keyframes_block<'i>(
    self,
    name: String,
    input: &mut Parser<'i>,
  ) -> Result<StyleSheetFragment, ParseError<StyleSheetParseError>> {
    let mut parser = KeyframeRuleParser { lossy: self.lossy };
    let mut keyframes = Vec::new();
    let mut ignored = Vec::new();

    for keyframe in recover(
      StyleSheetParser::new(input, &mut parser),
      self.lossy,
      &mut ignored,
    ) {
      keyframes.push(keyframe?);
    }
    for keyframe in &mut keyframes {
      ignored.extend(take(&mut keyframe.declarations.ignored));
    }

    let mut rule = KeyframesRule::builder()
      .name(name)
      .keyframes(keyframes)
      .build();
    rule.media_queries = self.media_queries.to_vec();

    Ok(StyleSheetFragment {
      keyframes: vec![rule],
      ignored,
      ..StyleSheetFragment::default()
    })
  }

  fn parse_style_rule_block<'i>(
    self,
    selectors: SelectorList<SelectorImpl>,
    input: &mut Parser<'i>,
  ) -> Result<StyleSheetFragment, ParseError<StyleSheetParseError>> {
    self.parse_nested_body(selectors, input, true)
  }

  /// The body of a conditional group rule or `@layer` nested in a rule on `parent_selectors`,
  /// which takes rules only in a nested `@scope`'s body, as Blink's
  /// `ConsumeRuleListOrNestedDeclarationList` reads declarations only under a style rule.
  fn parse_group_rule_block<'i>(
    self,
    parent_selectors: &SelectorList<SelectorImpl>,
    input: &mut Parser<'i>,
  ) -> Result<StyleSheetFragment, ParseError<StyleSheetParseError>> {
    self.parse_nested_body(parent_selectors.clone(), input, !self.scope_body)
  }

  fn parse_nested_body<'i>(
    self,
    selectors: SelectorList<SelectorImpl>,
    input: &mut Parser<'i>,
    declarations: bool,
  ) -> Result<StyleSheetFragment, ParseError<StyleSheetParseError>> {
    let mut parser = NestedStyleRuleParser {
      parent_selectors: selectors.clone(),
      context: self,
      declarations,
    };

    self.parse_rule_body(selectors, input, &mut parser)
  }

  /// The rules a body `parser` reads gives: each run of its declarations a rule on `selectors`, in
  /// order with the rules nested in it.
  fn parse_rule_body<'i>(
    self,
    selectors: SelectorList<SelectorImpl>,
    input: &mut Parser<'i>,
    parser: &mut impl RuleBodyItemParser<'i, StyleRuleBodyItem, StyleSheetParseError>,
  ) -> Result<StyleSheetFragment, ParseError<StyleSheetParseError>> {
    let mut declarations = RuleDeclarations::default();
    let mut fragment = StyleSheetFragment::default();
    let mut ignored = Vec::new();

    for item in recover(RuleBodyParser::new(input, parser), self.lossy, &mut ignored) {
      match item? {
        StyleRuleBodyItem::Declarations(block) => declarations.push(*block),
        StyleRuleBodyItem::Rules(nested_rules) => {
          fragment.ignored.extend(declarations.take_ignored());
          if !declarations.is_empty() {
            fragment
              .rules
              .push(self.rule(selectors.clone(), take(&mut declarations)));
          }
          fragment.extend(nested_rules);
        }
      }
    }
    fragment.ignored.extend(declarations.take_ignored());
    fragment.ignored.extend(ignored);

    if !declarations.is_empty() {
      fragment.rules.push(self.rule(selectors, declarations));
    }

    Ok(fragment)
  }

  fn parse_nested_at_rule_block<'i>(
    self,
    parent_selectors: &SelectorList<SelectorImpl>,
    prelude: AtRulePrelude,
    input: &mut Parser<'i>,
  ) -> Result<StyleSheetFragment, ParseError<StyleSheetParseError>> {
    match prelude {
      AtRulePrelude::Layer(layer_names) => {
        ensure_single_layer_name(&layer_names)?;
        let Some(layer_name) = layer_names.first() else {
          return Ok(StyleSheetFragment::default());
        };
        let nested_layer = extend_layer_name(self.layer, layer_name)?;
        let mut fragment = NestingContext {
          layer: Some(&nested_layer),
          ..self
        }
        .parse_group_rule_block(parent_selectors, input)?;

        fragment.declared_layers.insert(0, nested_layer);
        Ok(fragment)
      }
      AtRulePrelude::Media(media_query) => {
        let mut media_queries = self.media_queries.to_vec();
        media_queries.push(media_query);

        NestingContext {
          media_queries: &media_queries,
          ..self
        }
        .parse_group_rule_block(parent_selectors, input)
      }
      AtRulePrelude::Supports(true) => self.parse_group_rule_block(parent_selectors, input),
      AtRulePrelude::Supports(false) => Ok(skip_block(input)),
      AtRulePrelude::Container(query) => {
        let mut container_queries = self.container_queries.to_vec();

        container_queries.push(Arc::new(query));
        NestingContext {
          container_queries: &container_queries,
          ..self
        }
        .parse_group_rule_block(parent_selectors, input)
      }
      // A nested `@scope` roots at the elements its start selects relative to the rule around it,
      // or without a start at its `<style>` element's parent as Blink's `ConsumeScopeRule` makes an
      // implicit scope at any nesting, and its declarations apply to the root itself.
      // <https://drafts.csswg.org/css-cascade-6/#nested-scope>
      AtRulePrelude::Scope(scope) => {
        let owner = scope.start.is_none().then_some(self.style_owner).flatten();
        let scope = Arc::new(Scope {
          start: scope
            .start
            .map(|start| start.replace_parent_selector(parent_selectors)),
          end: scope.end,
          owner,
          parent: self.scope.cloned(),
        });
        NestingContext {
          scope: Some(&scope),
          scope_body: true,
          ..self
        }
        .parse_style_rule_block(scope_root_selectors()?, input)
      }
      AtRulePrelude::Unknown(at_rule) => {
        skip_block(input);
        Ok(StyleSheetFragment::ignoring(*at_rule))
      }
      AtRulePrelude::Theme => self.parse_theme_block(input),
      #[cfg(feature = "font-face")]
      AtRulePrelude::FontFace => Err(ParseError::custom(
        StyleSheetParseErrorKind::UnsupportedNestedAtRule,
      )),
      AtRulePrelude::Keyframes(_)
      | AtRulePrelude::Property(_)
      | AtRulePrelude::Page(_)
      | AtRulePrelude::TailwindImport { .. }
      | AtRulePrelude::StyleOwner(_)
      | AtRulePrelude::Apply(..) => Err(ParseError::custom(
        StyleSheetParseErrorKind::UnsupportedNestedAtRule,
      )),
    }
  }
}

/// Whether the Tailwind v4 entry point `target` brings Preflight, or `None` when `target` is not one.
/// <https://tailwindcss.com/docs/preflight#disabling-preflight>
fn tailwind_entry_preflight(target: &str) -> Option<bool> {
  match target.strip_prefix("tailwindcss")? {
    "" | "/index" | "/index.css" | "/preflight" | "/preflight.css" => Some(true),
    "/theme" | "/theme.css" | "/utilities" | "/utilities.css" => Some(false),
    _ => None,
  }
}

/// The internal at-rule an HTML `<style>` element's CSS starts with to name its owner, which the
/// parser then gives each `@scope` rule without a start, and ends with as `none`.
pub(crate) const STYLE_OWNER_RULE: &str = "-takumi-style-owner";

/// Parses an at-rule's prelude, an `@scope` start `scope_start` relative to the rule around it.
fn parse_at_rule_prelude<'i>(
  name: CowRcStr<'i>,
  input: &mut Parser<'i>,
  scope_start: ParseRelative,
) -> Result<AtRulePrelude, ParseError<StyleSheetParseError>> {
  if name.eq_ignore_ascii_case("layer") {
    if input.is_exhausted() {
      return Ok(AtRulePrelude::Layer(vec![vec![LayerName::anonymous()]]));
    }
    return Ok(AtRulePrelude::Layer(
      input.parse_comma_separated(parse_layer_name)?,
    ));
  }

  if name.eq_ignore_ascii_case("keyframes") {
    return Ok(AtRulePrelude::Keyframes(
      input.expect_ident_or_string()?.to_string(),
    ));
  }

  if name.eq_ignore_ascii_case("media") {
    return MediaQueryList::parse(input).map(AtRulePrelude::Media);
  }

  if name.eq_ignore_ascii_case("container") {
    return ContainerQuery::parse(input).map(AtRulePrelude::Container);
  }

  if name.eq_ignore_ascii_case("scope") {
    let selectors = |input: &mut Parser<'i>, relative| {
      // Pseudo-elements are invalid in a scope's root and limit selectors.
      input.parse_nested_block(|input| {
        SelectorList::parse_disallow_pseudo(&TakumiSelectorParser, input, relative)
      })
    };
    let start = input
      .try_parse(|input| {
        input.expect_parenthesis_block()?;
        selectors(input, scope_start)
      })
      .ok();
    let end = input
      .try_parse(|input| {
        input.expect_ident_matching("to")?;
        input.expect_parenthesis_block()?;
        selectors(input, ParseRelative::ForScope)
      })
      .ok();

    input.expect_exhausted()?;
    return Ok(AtRulePrelude::Scope(Scope {
      start,
      end,
      owner: None,
      parent: None,
    }));
  }

  if name.eq_ignore_ascii_case(STYLE_OWNER_RULE) {
    if input
      .try_parse(|input| input.expect_ident_matching("none"))
      .is_ok()
    {
      input.expect_exhausted()?;
      return Ok(AtRulePrelude::StyleOwner(None));
    }

    let owner = input.expect_integer()?;

    input.expect_exhausted()?;
    return u32::try_from(owner)
      .map(|owner| AtRulePrelude::StyleOwner(Some(owner)))
      .map_err(|_| ParseError::unexpected_token());
  }

  if name.eq_ignore_ascii_case("supports") {
    return parse_supports_condition(input).map(AtRulePrelude::Supports);
  }

  if name.eq_ignore_ascii_case("import") {
    let target = match input.next()? {
      Token::QuotedString(value) => value.clone(),
      _ => return Err(ParseError::unexpected_token()),
    };

    let Some(preflight) = tailwind_entry_preflight(&target) else {
      return Err(ParseError::from_basic_kind(
        BasicParseErrorKind::AtRuleInvalid,
      ));
    };

    // Modifiers such as `layer(base)`, `source(none)` and `important` change nothing here.
    while input.next().is_ok() {}

    return Ok(AtRulePrelude::TailwindImport { preflight });
  }

  if name.eq_ignore_ascii_case("apply") {
    let mut source = String::new();
    let mut start = input.position();
    loop {
      let before = input.position();
      match input.next_including_whitespace_and_comments() {
        Ok(Token::Comment(_)) => {
          source.push_str(input.slice(start..before));
          source.push(' ');
          start = input.position();
        }
        Ok(_) => {}
        Err(_) => break,
      }
    }
    source.push_str(input.slice_from(start));

    let (block, theme_keyframes) = expand_apply(&source)
      .ok_or_else(|| ParseError::custom(StyleSheetParseErrorKind::InvalidApplyUtility))?;

    return Ok(AtRulePrelude::Apply(Box::new(block), theme_keyframes));
  }

  if name.eq_ignore_ascii_case("theme") {
    while !input.is_exhausted() {
      let modifier = input.expect_ident()?.clone();
      match_ignore_ascii_case! {&modifier,
        "reference" | "default" | "static" | "inline" => {},
        _ => return Err(ParseError::unexpected_token()),
      }
    }
    return Ok(AtRulePrelude::Theme);
  }

  #[cfg(feature = "font-face")]
  if name.eq_ignore_ascii_case("font-face") {
    input.expect_exhausted()?;

    return Ok(AtRulePrelude::FontFace);
  }

  // Blink's `ConsumePageRule` drops a rule whose selector `ParsePageSelector` rejects.
  if name.eq_ignore_ascii_case("page")
    && let Ok(selector) = input.try_parse(|input| PageSelector::parse(input).ok_or(()))
  {
    return Ok(AtRulePrelude::Page(selector));
  }

  if name.eq_ignore_ascii_case("property") {
    let property_name = input.expect_ident_or_string()?.to_string();
    if !property_name.starts_with("--") {
      return Err(ParseError::custom(
        StyleSheetParseErrorKind::PropertyNameMustBeCustomProperty,
      ));
    }
    return Ok(AtRulePrelude::Property(property_name));
  }

  let start = input.position();

  while input.next().is_ok() {}

  Ok(AtRulePrelude::Unknown(Box::new(IgnoredDeclaration::new(
    format!("@{name}"),
    input.slice_from(start),
    IgnoredReason::UnknownAtRule,
    DeclarationOrigin::StyleSheet,
  ))))
}

pub(crate) fn parse_layer_name<'i>(
  input: &mut Parser<'i>,
) -> Result<LayerPath, ParseError<StyleSheetParseError>> {
  let mut segments = Vec::new();

  loop {
    let segment = match input.next()? {
      Token::Ident(value) if !is_reserved_custom_ident(value) => value.to_string(),
      _ => return Err(ParseError::unexpected_token()),
    };
    segments.push(LayerName::Named(segment));
    if segments.len() > MAX_LAYER_DEPTH {
      return Err(ParseError::unexpected_token());
    }

    if input.try_parse(|input| input.expect_delim('.')).is_err() {
      break;
    }
  }

  Ok(segments)
}

/// Reads a block whose rules do not apply to its end, which the parser needs
/// before it accepts the block.
fn skip_block(input: &mut Parser<'_>) -> StyleSheetFragment {
  while input.next_including_whitespace_and_comments().is_ok() {}

  StyleSheetFragment::default()
}

fn extend_layer_name(
  current_layer: Option<&LayerPath>,
  layer_name: &[LayerName],
) -> Result<LayerPath, ParseError<StyleSheetParseError>> {
  if current_layer.map_or(0, Vec::len) + layer_name.len() > MAX_LAYER_DEPTH {
    return Err(ParseError::unexpected_token());
  }
  let mut combined = current_layer.cloned().unwrap_or_default();

  combined.extend(layer_name.iter().cloned());
  Ok(combined)
}

fn ensure_single_layer_name(
  layer_names: &[LayerPath],
) -> Result<(), ParseError<StyleSheetParseError>> {
  if layer_names.len() <= 1 {
    return Ok(());
  }

  Err(ParseError::custom(
    StyleSheetParseErrorKind::LayerBlockMultipleNames,
  ))
}

impl RuleParser {
  /// How a selector at this level reads: relative to the scope root inside an `@scope`.
  fn relative(&self) -> ParseRelative {
    if self.scoped {
      ParseRelative::ForScope
    } else {
      ParseRelative::No
    }
  }
}

impl<'i> QualifiedRuleParser<'i> for RuleParser {
  type Prelude = SelectorList<SelectorImpl>;
  type QualifiedRule = StyleSheetFragment;
  type Error = StyleSheetParseError;

  fn parse_prelude(
    &mut self,
    input: &mut Parser<'i>,
  ) -> Result<Self::Prelude, ParseError<Self::Error>> {
    SelectorList::parse(&TakumiSelectorParser, input, self.relative())
  }

  fn parse_block(
    &mut self,
    selectors: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i>,
  ) -> Result<Self::QualifiedRule, ParseError<Self::Error>> {
    self.context().parse_style_rule_block(selectors, input)
  }
}

impl<'i> AtRuleParser<'i> for RuleParser {
  type Prelude = AtRulePrelude;
  type AtRule = StyleSheetFragment;
  type Error = StyleSheetParseError;

  fn parse_prelude(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i>,
  ) -> Result<Self::Prelude, ParseError<Self::Error>> {
    parse_at_rule_prelude(name, input, self.relative())
  }

  fn parse_block(
    &mut self,
    prelude: Self::Prelude,
    _location: &ParserState,
    input: &mut Parser<'i>,
  ) -> Result<Self::AtRule, ParseError<Self::Error>> {
    match prelude {
      AtRulePrelude::Layer(layer_names) => {
        ensure_single_layer_name(&layer_names)?;
        let declared_layers = self.declared_layers(&layer_names)?;
        let Some(nested_layer) = declared_layers.first().cloned() else {
          return Ok(StyleSheetFragment {
            declared_layers,
            ..StyleSheetFragment::default()
          });
        };
        let mut fragment = self.nested(Some(nested_layer)).parse_fragment(input)?;
        fragment.declared_layers.splice(0..0, declared_layers);
        Ok(fragment)
      }
      AtRulePrelude::Keyframes(name) => self.context().parse_keyframes_block(name, input),
      AtRulePrelude::Theme => self.context().parse_theme_block(input),
      // `@import` and `@apply` end at their semicolon; a block after them is invalid.
      AtRulePrelude::TailwindImport { .. }
      | AtRulePrelude::StyleOwner(_)
      | AtRulePrelude::Apply(..) => Err(ParseError::custom(
        StyleSheetParseErrorKind::UnsupportedNestedAtRule,
      )),
      AtRulePrelude::Media(media_query) => {
        let mut fragment = self
          .nested(self.current_layer.clone())
          .parse_fragment(input)?;

        for rule in &mut fragment.rules {
          rule.media_queries.push(media_query.clone());
        }
        for keyframes in &mut fragment.keyframes {
          keyframes.media_queries.push(media_query.clone());
        }
        for property_rule in &mut fragment.property_rules {
          property_rule.media_queries.push(media_query.clone());
        }
        #[cfg(feature = "font-face")]
        for font_face in &mut fragment.font_faces {
          font_face.media_queries.push(media_query.clone());
        }
        for page_rule in &mut fragment.page_rules {
          page_rule.media_queries.push(media_query.clone());
        }

        Ok(fragment)
      }
      AtRulePrelude::Container(query) => {
        let query = Arc::new(query);
        let mut fragment = self
          .nested(self.current_layer.clone())
          .parse_fragment(input)?;

        for rule in &mut fragment.rules {
          rule.container_queries.push(Arc::clone(&query));
        }
        Ok(fragment)
      }
      AtRulePrelude::Scope(scope) => {
        let owner = scope.start.is_none().then_some(self.style_owner).flatten();
        let scope = Arc::new(Scope { owner, ..scope });
        let mut body = ScopeBodyParser(Self {
          scoped: true,
          ..self.nested(self.current_layer.clone())
        });
        let mut fragment =
          self
            .context()
            .parse_rule_body(scope_root_selectors()?, input, &mut body)?;

        for rule in &mut fragment.rules {
          rule.scope = Some(match &rule.scope {
            Some(inner) => inner.within(&scope),
            None => Arc::clone(&scope),
          });
        }
        Ok(fragment)
      }
      AtRulePrelude::Supports(true) => self
        .nested(self.current_layer.clone())
        .parse_fragment(input),
      AtRulePrelude::Supports(false) => Ok(skip_block(input)),
      AtRulePrelude::Unknown(at_rule) => {
        skip_block(input);
        Ok(StyleSheetFragment::ignoring(*at_rule))
      }
      AtRulePrelude::Property(name) => Ok(StyleSheetFragment {
        property_rules: vec![parse_property_rule(name, input)?],
        ..StyleSheetFragment::default()
      }),
      #[cfg(feature = "font-face")]
      AtRulePrelude::FontFace => Ok(StyleSheetFragment {
        font_faces: FontFaceRule::parse(input)
          .map(|rule| FontFaceRule {
            layer: self.current_layer.clone(),
            ..rule
          })
          .into_iter()
          .collect(),
        ..StyleSheetFragment::default()
      }),
      AtRulePrelude::Page(selector) => Ok(StyleSheetFragment {
        page_rules: vec![PageRule {
          layer: self.current_layer.clone(),
          ..PageRule::parse(selector, input)
        }],
        ..StyleSheetFragment::default()
      }),
    }
  }

  fn rule_without_block(
    &mut self,
    prelude: Self::Prelude,
    _start: &ParserState,
  ) -> Result<Self::AtRule, ()> {
    match prelude {
      AtRulePrelude::Layer(layer_names) => Ok(StyleSheetFragment {
        declared_layers: self.declared_layers(&layer_names).map_err(|_| ())?,
        ..StyleSheetFragment::default()
      }),
      AtRulePrelude::StyleOwner(owner) if self.top_level => {
        self.style_owner = owner;
        Ok(StyleSheetFragment::default())
      }
      AtRulePrelude::TailwindImport { preflight } => {
        if !self.top_level {
          return Err(());
        }

        Ok(StyleSheetFragment {
          preflight,
          ..StyleSheetFragment::default()
        })
      }
      AtRulePrelude::Unknown(at_rule) => Ok(StyleSheetFragment::ignoring(*at_rule)),
      _ => Err(()),
    }
  }
}

/// The selectors of the declarations directly in an `@scope`, which apply to its root with no
/// specificity. <https://drafts.csswg.org/css-cascade-6/#scoped-declarations>
fn scope_root_selectors() -> Result<SelectorList<SelectorImpl>, ParseError<StyleSheetParseError>> {
  SelectorList::parse(
    &TakumiSelectorParser,
    &mut Parser::new(":where(:scope)"),
    ParseRelative::No,
  )
  .map_err(ParseError::into)
}

/// The direct body of a top-level `@scope`, which takes the rules a style sheet does and, as
/// Blink's `ConsumeBlockContents` reads it, declarations.
struct ScopeBodyParser(RuleParser);

impl<'i> DeclarationParser<'i> for ScopeBodyParser {
  type Declaration = StyleRuleBodyItem;
  type Error = StyleSheetParseError;

  fn parse_value(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i>,
    state: &ParserState,
  ) -> Result<Self::Declaration, ParseError<Self::Error>> {
    parse_body_declaration(name, input, state)
  }
}

impl<'i> QualifiedRuleParser<'i> for ScopeBodyParser {
  type Prelude = SelectorList<SelectorImpl>;
  type QualifiedRule = StyleRuleBodyItem;
  type Error = StyleSheetParseError;

  fn parse_prelude(
    &mut self,
    input: &mut Parser<'i>,
  ) -> Result<Self::Prelude, ParseError<Self::Error>> {
    QualifiedRuleParser::parse_prelude(&mut self.0, input)
  }

  fn parse_block(
    &mut self,
    selectors: Self::Prelude,
    location: &ParserState,
    input: &mut Parser<'i>,
  ) -> Result<Self::QualifiedRule, ParseError<Self::Error>> {
    QualifiedRuleParser::parse_block(&mut self.0, selectors, location, input)
      .map(StyleRuleBodyItem::Rules)
  }
}

impl<'i> AtRuleParser<'i> for ScopeBodyParser {
  type Prelude = AtRulePrelude;
  type AtRule = StyleRuleBodyItem;
  type Error = StyleSheetParseError;

  fn parse_prelude(
    &mut self,
    name: CowRcStr<'i>,
    input: &mut Parser<'i>,
  ) -> Result<Self::Prelude, ParseError<Self::Error>> {
    AtRuleParser::parse_prelude(&mut self.0, name, input)
  }

  fn parse_block(
    &mut self,
    prelude: Self::Prelude,
    location: &ParserState,
    input: &mut Parser<'i>,
  ) -> Result<Self::AtRule, ParseError<Self::Error>> {
    AtRuleParser::parse_block(&mut self.0, prelude, location, input).map(StyleRuleBodyItem::Rules)
  }

  fn rule_without_block(
    &mut self,
    prelude: Self::Prelude,
    start: &ParserState,
  ) -> Result<Self::AtRule, ()> {
    self
      .0
      .rule_without_block(prelude, start)
      .map(StyleRuleBodyItem::Rules)
  }
}

impl<'i> RuleBodyItemParser<'i, StyleRuleBodyItem, StyleSheetParseError> for ScopeBodyParser {
  fn parse_qualified(&self) -> bool {
    true
  }

  fn parse_declarations(&self) -> bool {
    true
  }
}

/// Defines a stylesheet with rules, keyframes, and property rules.
#[derive(Debug, Clone, Default)]
pub struct StyleSheet {
  /// Style rules in source order, shared by clones.
  rules: Arc<[CssRule]>,
  /// Index over `rules`, built with them; `rules` never changes afterwards.
  rule_index: Arc<RuleIndex>,
  /// `@keyframes` rules.
  pub(crate) keyframes: Vec<KeyframesRule>,
  /// `@property` rules.
  pub(crate) property_rules: Vec<PropertyRule>,
  /// `@font-face` rules that make a face, in source order.
  #[cfg(feature = "font-face")]
  pub(crate) font_faces: Vec<FontFaceRule>,
  /// `@page` rules, in source order.
  pub(crate) page_rules: Vec<PageRule>,
  /// Number of distinct cascade layers.
  pub(crate) layer_count: usize,
  /// Widths from unconditional `:root` `--breakpoint-*` declarations, which
  /// gate `tw` variants before the cascade runs and so resolve at parse time.
  pub(crate) breakpoints: BreakpointOverrides,
  /// Values of unconditional `:root` `--animate-*` declarations by token, which name the theme
  /// `@keyframes` the `animate-*` utilities keep.
  pub(crate) animate_theme: HashMap<String, String>,
  /// The declarations and at-rules parsing dropped, each once.
  ignored: Box<[IgnoredDeclaration]>,
}

impl From<Vec<KeyframesRule>> for StyleSheet {
  fn from(keyframes: Vec<KeyframesRule>) -> Self {
    Self {
      keyframes,
      ..Default::default()
    }
  }
}

impl From<Vec<PropertyRule>> for StyleSheet {
  fn from(property_rules: Vec<PropertyRule>) -> Self {
    Self {
      property_rules,
      ..Default::default()
    }
  }
}

/// [Tailwind's Preflight](https://github.com/tailwindlabs/tailwindcss/blob/main/packages/tailwindcss/preflight.css),
/// without the rules for form controls, pseudo-elements, `abbr[title]`, and `summary`.
const PREFLIGHT_CSS: &str = r"
@layer base {
  *,
  ::before,
  ::after {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
    border: 0 solid;
  }

  :root {
    line-height: 1.5;
    tab-size: 4;
  }

  hr {
    height: 0;
    color: inherit;
    border-top-width: 1px;
  }

  h1, h2, h3, h4, h5, h6 {
    font-size: inherit;
    font-weight: inherit;
  }

  a {
    color: inherit;
    text-decoration: inherit;
  }

  b, strong {
    font-weight: bolder;
  }

  code, kbd, samp, pre {
    font-size: 1em;
  }

  small {
    font-size: 80%;
  }

  sub, sup {
    font-size: 75%;
    line-height: 0;
    position: relative;
    vertical-align: baseline;
  }

  sub {
    bottom: -0.25em;
  }

  sup {
    top: -0.5em;
  }

  table {
    text-indent: 0;
    border-color: inherit;
    border-collapse: collapse;
  }

  ol, ul, menu {
    list-style: none;
  }

  img, svg {
    display: block;
    vertical-align: middle;
  }

  img {
    max-width: 100%;
    height: auto;
  }

  [hidden]:where(:not([hidden='until-found'])) {
    display: none !important;
  }
}
";

impl StyleSheet {
  /// Style rules in source order.
  pub(crate) fn rules(&self) -> &[CssRule] {
    &self.rules
  }

  /// Whether a rule is in an `@container` rule.
  pub(crate) fn has_container_queries(&self) -> bool {
    self
      .rules
      .iter()
      .any(|rule| !rule.container_queries.is_empty())
  }

  /// Collects unique resource URLs referenced by the declarations of its rules, keyframes, and
  /// page-margin rules.
  pub fn image_urls(&self) -> impl Iterator<Item = &str> {
    let rules = self
      .rules
      .iter()
      .flat_map(|rule| [&rule.normal_declarations, &rule.important_declarations]);
    let keyframes = self
      .keyframes
      .iter()
      .flat_map(|rule| &rule.keyframes)
      .map(|keyframe| &keyframe.declarations);
    let urls: Xxh3HashSet<&str> = rules
      .chain(keyframes)
      .flat_map(StyleDeclarationBlock::image_urls)
      .chain(self.page_rules.iter().flat_map(PageRule::image_urls))
      .collect();

    urls.into_iter()
  }

  /// The index over [`Self::rules`].
  pub(crate) fn rule_index(&self) -> &RuleIndex {
    &self.rule_index
  }

  /// The `@property` registrations declared by this stylesheet.
  pub(crate) fn property_rules(&self) -> &[PropertyRule] {
    &self.property_rules
  }

  /// The `@font-face` rules that make a face, in source order.
  #[cfg(feature = "font-face")]
  pub(crate) fn font_faces(&self) -> &[FontFaceRule] {
    &self.font_faces
  }

  /// The `@page` descriptors that win for a `kind` page named `name` in `context`, with `@media`
  /// read at `viewport`.
  pub fn page_descriptors(
    &self,
    viewport: Viewport,
    kind: PageKind,
    name: &PageName,
    context: &PageContext,
  ) -> PageDescriptors {
    PageRule::cascade(
      &self.page_rules,
      self.layer_count,
      viewport,
      kind,
      name,
      context,
    )
  }

  /// The declarations that style the boxes of a `kind` page named `name` in `context`, with
  /// `@media` read at `viewport`.
  pub fn page_styles(
    &self,
    viewport: Viewport,
    kind: PageKind,
    name: &PageName,
    context: &PageContext,
  ) -> PageStyles {
    PageRule::styles(
      &self.page_rules,
      self.layer_count,
      viewport,
      kind,
      name,
      context,
    )
  }

  /// Whether an `@page` rule nests a page-margin rule.
  pub fn has_page_margin_rules(&self) -> bool {
    self.page_rules.iter().any(|rule| !rule.margins.is_empty())
  }

  /// Whether a page-margin rule's `content` reads `counter(pages)`, the document's page count.
  pub fn page_margins_count_pages(&self) -> bool {
    self.page_rules.iter().any(PageRule::counts_pages)
  }

  /// The page names the `@page` rules select, each once, in the order they first appear.
  pub fn page_names(&self) -> Vec<PageName> {
    let mut names: Vec<PageName> = Vec::new();

    for name in self.page_rules.iter().map(|rule| rule.selector.name()) {
      if name.is_named() && !names.contains(name) {
        names.push(name.clone());
      }
    }
    names
  }

  /// The declarations and at-rules parsing dropped, each once.
  pub fn ignored_declarations(&self) -> &[IgnoredDeclaration] {
    &self.ignored
  }

  /// Extends the stylesheet with keyframes.
  pub fn extend_keyframes(&mut self, keyframes: Vec<KeyframesRule>) {
    self.keyframes.extend(keyframes);
  }

  /// Parses a list of stylesheets, each on its own, into one sheet in their order.
  pub fn parse_list<I, S>(stylesheets: I) -> Result<Self, StyleSheetParseError>
  where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
  {
    Self::parse_with_mode(stylesheets, false)
  }

  /// Parses a list of stylesheets while discarding invalid rules.
  pub fn parse_list_loosy<I, S>(stylesheets: I) -> Self
  where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
  {
    Self::parse_with_mode(stylesheets, true).unwrap_or_default()
  }

  /// Parses a list of owned stylesheets while discarding invalid rules.
  pub fn parse_owned_list_loosy(stylesheets: Vec<String>) -> Self {
    Self::parse_list_loosy(stylesheets)
  }

  /// Parses a stylesheet.
  pub fn parse(css: &str) -> Result<Self, StyleSheetParseError> {
    Self::parse_list([css])
  }

  /// Parses a stylesheet while discarding invalid rules.
  pub fn parse_loosy(css: &str) -> Self {
    Self::parse_list_loosy([css])
  }

  /// Parses each of `stylesheets` on its own, as Blink's `CSSParserImpl::ParseStyleSheet` parses
  /// each `StyleSheetContents`, so an unclosed block or comment ends with its sheet.
  fn parse_with_mode<S: AsRef<str>>(
    stylesheets: impl IntoIterator<Item = S>,
    lossy: bool,
  ) -> Result<Self, StyleSheetParseError> {
    let mut fragment = StyleSheetFragment::default();

    for css in stylesheets {
      let mut parser = Parser::new(css.as_ref());
      let mut rule_parser = RuleParser {
        current_layer: None,
        style_owner: None,
        lossy,
        scoped: false,
        top_level: true,
      };

      for parsed in StyleSheetParser::new(&mut parser, &mut rule_parser) {
        match parsed {
          Ok(parsed) => fragment.extend(parsed),
          Err((_, source, location)) if lossy => log_ignored(source, location),
          Err((error, context, _)) => {
            return Err(StyleSheetParseError::from_parse_error(context, error));
          }
        }
      }
    }

    let StyleSheetFragment {
      mut rules,
      keyframes,
      theme_keyframes,
      property_rules,
      #[cfg(feature = "font-face")]
      mut font_faces,
      mut page_rules,
      mut declared_layers,
      preflight,
      ignored,
    } = fragment;

    if preflight {
      static PREFLIGHT_RULES: LazyLock<Arc<[CssRule]>> =
        LazyLock::new(|| StyleSheet::parse_loosy(PREFLIGHT_CSS).rules);

      let mut preflight_rules = PREFLIGHT_RULES.to_vec();
      declared_layers.splice(
        0..0,
        preflight_rules.iter().filter_map(|rule| rule.layer.clone()),
      );
      preflight_rules.append(&mut rules);
      rules = preflight_rules;
    }

    let mut layers = LayerTree::default();
    let mut layer_ids = HashMap::<&LayerPath, usize>::new();
    let used_layers = rules.iter().filter_map(|rule| rule.layer.as_ref());
    #[cfg(feature = "font-face")]
    let used_layers = used_layers.chain(font_faces.iter().filter_map(|rule| rule.layer.as_ref()));
    let used_layers = used_layers.chain(page_rules.iter().filter_map(|rule| rule.layer.as_ref()));

    for layer_name in declared_layers.iter().chain(used_layers) {
      if !layer_ids.contains_key(layer_name) {
        layer_ids.insert(layer_name, layers.insert(layer_name));
      }
    }
    let orders = layers.orders();
    let layer_order: HashMap<LayerPath, usize> = layer_ids
      .into_iter()
      .map(|(layer_name, layer)| (layer_name.clone(), orders[layer]))
      .collect();

    for rule in &mut rules {
      rule.layer_order = rule
        .layer
        .as_ref()
        .and_then(|layer_name| layer_order.get(layer_name).copied());
    }

    #[cfg(feature = "font-face")]
    for font_face in &mut font_faces {
      font_face.layer_order = font_face
        .layer
        .as_ref()
        .and_then(|layer_name| layer_order.get(layer_name).copied());
    }

    for page_rule in &mut page_rules {
      page_rule.layer_order = page_rule
        .layer
        .as_ref()
        .and_then(|layer_name| layer_order.get(layer_name).copied());
    }

    rules.retain(|rule| {
      !rule.normal_declarations.declarations.is_empty()
        || !rule.important_declarations.declarations.is_empty()
    });

    let animate_theme = root_theme_values(&rules, "--animate-", |value| Some(value.to_owned()));
    let mut applied_theme_keyframes = Vec::new();

    theme_keyframes.resolve_into(&animate_theme, &mut applied_theme_keyframes);

    Ok(Self {
      breakpoints: root_theme_values(&rules, "--breakpoint-", |value| {
        // Only units `Breakpoint::matches` resolves; anything else would
        // shadow the built-in width with a dead one.
        Length::from_css_str(value)
          .ok()
          .filter(|width| matches!(width, Length::Rem(_) | Length::Px(_) | Length::Vw(_)))
      }),
      animate_theme,
      rule_index: Arc::new(RuleIndex::build(&rules)),
      rules: rules.into(),
      keyframes: applied_theme_keyframes
        .into_iter()
        .cloned()
        .chain(keyframes)
        .collect(),
      property_rules,
      #[cfg(feature = "font-face")]
      font_faces,
      page_rules,
      layer_count: layer_order.len(),
      ignored: IgnoredDeclaration::unique(&ignored).into(),
    })
  }
}

/// The values `parse` reads from unconditional `:root` `<prefix><token>` declarations, by token.
/// Variants gate at parse time, so only statically-known declarations can re-size them.
fn root_theme_values<T>(
  rules: &[CssRule],
  prefix: &str,
  parse: impl Fn(&str) -> Option<T>,
) -> HashMap<String, T> {
  let mut values = HashMap::new();
  let mut winners = HashMap::<String, usize>::new();

  for rule in rules {
    if !rule.media_queries.is_empty()
      || !rule.container_queries.is_empty()
      || rule.selectors.to_css_string() != ":root"
    {
      continue;
    }

    // Unlayered declarations outrank every layer, so they sort above the
    // highest ordinal rather than below the lowest.
    let precedence = rule.layer_order.unwrap_or(usize::MAX);

    for declaration in rule.normal_declarations.declarations.iter() {
      if let StyleDeclaration::CustomProperty(name, value) = declaration
        && let Some(token) = name.strip_prefix(prefix)
        && let Some(parsed) = parse(value)
        && winners
          .get(token)
          .is_none_or(|winner| precedence >= *winner)
      {
        winners.insert(token.to_owned(), precedence);
        values.insert(token.to_owned(), parsed);
      }
    }
  }

  values
}

#[cfg(test)]
mod tests {
  use cssparser::ToCss;

  use super::*;
  use crate::{
    style::{Color, ColorInput, ComputedStyle, Length, Style, StyleDeclaration},
    viewport::{ColorScheme, MediaTarget, Viewport},
  };

  fn computed_style_from_declarations(declarations: &StyleDeclarationBlock) -> ComputedStyle {
    let mut style = Style::default();
    for declaration in &declarations.declarations {
      style.push(declaration.clone(), false);
    }
    style.inherit(&ComputedStyle::default())
  }

  fn selector_text(rule: &CssRule) -> String {
    rule.selectors.to_css_string()
  }

  fn parse_stylesheet(css: &str) -> StyleSheet {
    let result = StyleSheet::parse(css);
    assert!(result.is_ok(), "expected stylesheet to parse: {result:?}");
    result.unwrap_or_default()
  }

  fn parse_stylesheet_loosy(css: &str) -> StyleSheet {
    StyleSheet::parse_loosy(css)
  }

  fn assert_lossy_parse_keeps_single_valid_rule(css: &str) {
    let sheet = parse_stylesheet_loosy(css);
    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations)
        .box_data
        .width,
      Length::Px(100.0).into()
    );
  }

  #[test]
  fn image_urls_cover_rules_and_keyframes() {
    let sheet = parse_stylesheet_loosy(
      "a { background: url(a.png) } b { background-image: url(b.png) !important } \
       @keyframes k { to { background-image: url(c.png) } } d { background: url(a.png) }",
    );
    let mut urls: Vec<&str> = sheet.image_urls().collect();

    urls.sort_unstable();

    assert_eq!(urls, ["a.png", "b.png", "c.png"]);
  }

  #[test]
  fn test_parse_stylesheet() {
    let css = r#"
            .box {
                width: 100px;
                color: red;
            }
        "#;
    let sheet = parse_stylesheet(css);
    assert_eq!(sheet.rules.len(), 1);
    let rule = &sheet.rules[0];

    assert_eq!(rule.selectors.slice().len(), 1);
    assert_eq!(
      computed_style_from_declarations(&rule.normal_declarations)
        .box_data
        .width,
      Length::Px(100.0).into()
    );
  }

  #[test]
  fn test_parse_stylesheet_compound_selectors_specificity() {
    let sheet = parse_stylesheet(
      r#"
        div.box { width: 10px; }
        #hero .label { height: 20px; }
      "#,
    );
    assert_eq!(sheet.rules.len(), 2);
    assert_eq!(sheet.rules[0].selectors.slice().len(), 1);
    assert_eq!(sheet.rules[1].selectors.slice().len(), 1);
    assert!(sheet.rules[0].selectors.slice()[0].specificity() > 0);
    assert!(
      sheet.rules[1].selectors.slice()[0].specificity()
        > sheet.rules[0].selectors.slice()[0].specificity()
    );
  }

  #[test]
  fn test_parse_stylesheet_multiple_rules() {
    let sheet = parse_stylesheet(
      r#"
        .a { width: 10px; }
        .b { height: 20px; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 2);
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations)
        .box_data
        .width,
      Length::Px(10.0).into()
    );
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[1].normal_declarations)
        .box_data
        .height,
      Length::Px(20.0).into()
    );
  }

  #[test]
  fn test_parse_stylesheet_multiple_selectors_in_rule() {
    let sheet = parse_stylesheet(
      r#"
        .a, .b { width: 12px; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(sheet.rules[0].selectors.slice().len(), 2);
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations)
        .box_data
        .width,
      Length::Px(12.0).into()
    );
  }

  #[test]
  fn test_parse_stylesheet_universal_selector() {
    let sheet = parse_stylesheet(
      r#"
        * { width: 100px; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), "*");
    assert_eq!(sheet.rules[0].selectors.slice().len(), 1);
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations)
        .box_data
        .width,
      Length::Px(100.0).into()
    );
  }

  #[test]
  fn test_parse_stylesheet_important_declaration() {
    let sheet = parse_stylesheet(
      r#"
        .a { width: 10px !important; height: 20px; }
      "#,
    );

    let rule = &sheet.rules[0];
    assert_eq!(
      computed_style_from_declarations(&rule.important_declarations)
        .box_data
        .width,
      Length::Px(10.0).into()
    );
    assert_eq!(
      computed_style_from_declarations(&rule.normal_declarations)
        .box_data
        .height,
      Length::Px(20.0).into()
    );
  }

  #[test]
  fn test_parse_stylesheet_important_after_multi_token_values() {
    for declaration in [
      "background: red",
      "border-bottom: 8px solid red",
      "font: bold 12px serif",
      "box-shadow: 1px 1px red",
      "transform: scale(2)",
    ] {
      let normal = parse_stylesheet(&format!(".a {{ {declaration}; }}"));
      let important = parse_stylesheet(&format!(".a {{ {declaration} !important; }}"));

      assert_eq!(
        important.rules[0].important_declarations.declarations,
        normal.rules[0].normal_declarations.declarations,
        "`{declaration} !important` should mark what `{declaration}` sets important"
      );
    }
  }

  #[test]
  fn test_parse_stylesheet_shorthand_clears_prior_longhand() {
    let sheet = parse_stylesheet(
      r#"
        .a { padding-left: 4px; padding: 10px; }
      "#,
    );

    let declarations = &sheet.rules[0].normal_declarations;
    assert_eq!(declarations.declarations.len(), 5);
    assert_eq!(
      declarations.declarations[0],
      StyleDeclaration::padding_left(Length::Px(4.0))
    );
    assert_eq!(
      declarations.declarations[1],
      StyleDeclaration::padding_top(Length::Px(10.0))
    );
    assert_eq!(
      declarations.declarations[2],
      StyleDeclaration::padding_right(Length::Px(10.0))
    );
    assert_eq!(
      declarations.declarations[3],
      StyleDeclaration::padding_bottom(Length::Px(10.0))
    );
    assert_eq!(
      declarations.declarations[4],
      StyleDeclaration::padding_left(Length::Px(10.0))
    );
  }

  #[test]
  fn test_parse_stylesheet_webkit_alias_property() {
    let sheet = parse_stylesheet(
      r#"
        .a { -webkit-text-fill-color: rgb(255, 0, 0); }
      "#,
    );

    let style = computed_style_from_declarations(&sheet.rules[0].normal_declarations);
    assert_eq!(
      style.misc_inherited1_data.webkit_text_fill_color,
      Some(ColorInput::Value(Color([255, 0, 0, 255])))
    );
  }

  #[test]
  fn test_parse_stylesheet_unknown_property_does_not_drop_supported_declarations() {
    let sheet = parse_stylesheet(
      r#"
        .a { --local-token: 1; width: 14px; unsupported-prop: 2; height: 6px; }
      "#,
    );

    let style = computed_style_from_declarations(&sheet.rules[0].normal_declarations);
    assert_eq!(style.box_data.width, Length::Px(14.0).into());
    assert_eq!(style.box_data.height, Length::Px(6.0).into());
  }

  #[test]
  fn test_attribute_selector_rule_is_preserved() {
    let sheet = parse_stylesheet_loosy(
      r#"
        [data-kind="hero"] { width: 10px; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
  }

  #[test]
  fn test_parse_stylesheet_accepts_attribute_selector() {
    let result = StyleSheet::parse(
      r#"
        [data-kind="hero"] { width: 10px; }
      "#,
    );

    assert!(result.is_ok());
  }

  #[test]
  fn test_ignored_pseudo_selector_rule_is_kept_but_never_matches() {
    let sheet = parse_stylesheet_loosy(
      r#"
        .a:hover { width: 10px; }
        .a, .a:hover { height: 20px; }
        .a::before { color: red; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 3);
    assert_eq!(selector_text(&sheet.rules[0]), ".a:hover");
    assert_eq!(selector_text(&sheet.rules[1]), ".a, .a:hover");
    assert_eq!(selector_text(&sheet.rules[2]), ".a::before");
  }

  /// Stable Blink's `:lang()` takes one identifier and drops a rule with a list or a string.
  #[test]
  fn test_lang_pseudo_class_takes_one_identifier() {
    let sheet = parse_stylesheet_loosy(
      r#"
        .a:lang(zh-Hant) { color: blue; }
        .a:lang(en, fr) { color: red; }
        .a:lang("ja") { color: red; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ".a:lang(zh-Hant)");
  }

  #[test]
  fn test_is_and_where_selectors_are_accepted() {
    let sheet = parse_stylesheet_loosy(
      r#"
        .a:where(.b) div { background: red; }
        .a:is(.b, .c) { color: blue; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 2);
    assert_eq!(selector_text(&sheet.rules[0]), ".a:where(.b) div");
    assert_eq!(selector_text(&sheet.rules[1]), ".a:is(.b, .c)");
  }

  #[test]
  fn test_parse_keyframes_rule() {
    let sheet = parse_stylesheet(
      r#"
        @keyframes fade {
          from { opacity: 0; }
          50% { opacity: 0.5; }
          to { opacity: 1; }
        }
      "#,
    );

    assert!(sheet.rules.is_empty());
    assert_eq!(sheet.keyframes.len(), 1);
    assert_eq!(sheet.keyframes[0].name, "fade");
    assert_eq!(sheet.keyframes[0].keyframes.len(), 3);
    assert_eq!(sheet.keyframes[0].keyframes[0].offsets, vec![0.0]);
    assert_eq!(sheet.keyframes[0].keyframes[1].offsets, vec![0.5]);
    assert_eq!(sheet.keyframes[0].keyframes[2].offsets, vec![1.0]);
  }

  #[test]
  fn lossy_parse_drops_only_the_invalid_keyframe_declaration() {
    let sheet = parse_stylesheet_loosy("@keyframes fade { to { opacity: 1; width: bogus; } }");

    assert_eq!(sheet.keyframes.len(), 1);
    assert_eq!(sheet.keyframes[0].keyframes.len(), 1);
    assert_eq!(sheet.keyframes[0].keyframes[0].declarations.len(), 1);
  }

  /// <https://drafts.csswg.org/css-animations-1/#keyframes>
  #[test]
  fn a_keyframe_drops_an_important_declaration() {
    let sheet = parse_stylesheet_loosy(
      "@keyframes fade { to { opacity: 0 !important; --x: 1 !important; width: 10px; } }",
    );
    let declarations = &sheet.keyframes[0].keyframes[0].declarations;

    assert_eq!(declarations.len(), 1);
    assert!(matches!(
      declarations.iter().next(),
      Some(StyleDeclaration::Width(_))
    ));
  }

  #[test]
  fn prefers_color_scheme_matches_the_viewport_scheme() {
    let sheet = parse_stylesheet(
      r#"
        @media (prefers-color-scheme: dark) { .card { width: 100px; } }
        @media (prefers-color-scheme: light) { .card { width: 200px; } }
        @media (prefers-color-scheme) { .card { width: 300px; } }
      "#,
    );
    let dark = Viewport::new((800, 600)).with_color_scheme(ColorScheme::Dark);
    let light = Viewport::new((800, 600));
    let matches = |index: usize, viewport: Viewport| {
      sheet.rules[index]
        .media_queries
        .first()
        .is_some_and(|media| media.matches(viewport))
    };

    assert_eq!(sheet.rules.len(), 3);
    assert!(matches(0, dark) && !matches(0, light));
    assert!(matches(1, light) && !matches(1, dark));
    assert!(matches(2, light) && matches(2, dark));
  }

  #[test]
  fn user_preferences_hold_their_defaults() {
    let viewport = Viewport::new((800, 600));

    for (query, expected) in [
      ("(prefers-reduced-motion: no-preference)", true),
      ("(prefers-reduced-motion: reduce)", false),
      ("(prefers-reduced-motion)", false),
      ("(prefers-contrast: no-preference)", true),
      ("(prefers-contrast: more)", false),
      ("(prefers-contrast: less)", false),
      ("(forced-colors: none)", true),
      ("(forced-colors: active)", false),
      ("(forced-colors)", false),
      ("not (inverted-colors: inverted)", true),
      ("(inverted-colors: none)", true),
      ("(prefers-contrast: bogus)", false),
    ] {
      let sheet = parse_stylesheet(&format!("@media {query} {{ .a {{ width: 1px; }} }}"));

      assert_eq!(
        sheet.rules.first().is_some_and(|rule| {
          rule
            .media_queries
            .first()
            .is_some_and(|media| media.matches(viewport))
        }),
        expected,
        "{query}"
      );
    }
  }

  #[test]
  fn test_parse_media_rule_with_viewport_features() {
    let sheet = parse_stylesheet(
      r#"
        @media screen and (min-width: 600px) and (orientation: landscape) {
          .card { width: 100px; }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert!(sheet.keyframes.is_empty());
    assert!(
      sheet.rules[0]
        .media_queries
        .first()
        .is_some_and(|media| media.matches(Viewport::new((800, 600))))
    );
    assert!(
      !sheet.rules[0]
        .media_queries
        .first()
        .is_some_and(|media| media.matches(Viewport::new((500, 800))))
    );
  }

  #[test]
  fn test_parse_media_rule_ignores_feature_name_case() {
    for query in [
      "(MIN-WIDTH: 600px)",
      "(Min-Width: 600px)",
      "(WIDTH >= 600px)",
    ] {
      let sheet = parse_stylesheet(&format!("@media {query} {{ .card {{ width: 100px; }} }}"));
      let Some(media) = sheet.rules[0].media_queries.first() else {
        unreachable!("expected media queries on parsed rule: {query}");
      };

      assert!(media.matches(Viewport::new((800, 600))), "{query}");
      assert!(!media.matches(Viewport::new((400, 600))), "{query}");
    }
  }

  #[test]
  fn test_parse_media_rule_with_comma_list() {
    let sheet = parse_stylesheet(
      r#"
        @media (max-width: 480px), (min-width: 1024px) {
          .card { width: 100px; }
        }
      "#,
    );

    let Some(media) = sheet.rules[0].media_queries.first() else {
      unreachable!("expected media queries on parsed rule");
    };
    assert!(media.matches(Viewport::new((400, 800))));
    assert!(media.matches(Viewport::new((1280, 800))));
    assert!(!media.matches(Viewport::new((800, 800))));
  }

  #[test]
  fn test_parse_media_rule_with_range_syntax() {
    for (query, matching, non_matching) in [
      ("(width >= 600px)", (600, 400), (599, 400)),
      ("(width > 600px)", (601, 400), (600, 400)),
      ("(width <= 600px)", (600, 400), (601, 400)),
      ("(width < 600px)", (599, 400), (600, 400)),
      ("(width = 600px)", (600, 400), (601, 400)),
      ("(600px <= width)", (600, 400), (599, 400)),
      ("(600px > width)", (599, 400), (600, 400)),
      ("(height >= 400px)", (600, 400), (600, 399)),
      ("(400px < height)", (600, 401), (600, 400)),
      ("(width >= 40em)", (640, 400), (639, 400)),
      ("(400px < width <= 700px)", (700, 400), (400, 400)),
      ("(700px >= width > 400px)", (401, 400), (701, 400)),
      ("(width>=600px)", (600, 400), (599, 400)),
      ("(width >= calc(500px + 100px))", (600, 400), (599, 400)),
      ("(width)", (1, 400), (0, 400)),
    ] {
      let sheet = parse_stylesheet(&format!("@media {query} {{ .card {{ width: 100px; }} }}"));
      let Some(media) = sheet.rules[0].media_queries.first() else {
        unreachable!("expected media queries on parsed rule: {query}");
      };

      assert!(
        media.matches(Viewport::new(matching)),
        "{query} at {matching:?}"
      );
      assert!(
        !media.matches(Viewport::new(non_matching)),
        "{query} at {non_matching:?}"
      );
    }
  }

  #[test]
  fn test_parse_media_rule_with_resolution_and_aspect_ratio() {
    let dpr = |dpr: f32| Viewport::new((800, 400)).with_device_pixel_ratio(dpr);

    for (query, matching, non_matching) in [
      ("(resolution: 2dppx)", dpr(2.0), dpr(1.0)),
      ("(resolution >= 2x)", dpr(3.0), dpr(1.5)),
      ("(min-resolution: 192dpi)", dpr(3.0), dpr(1.0)),
      ("(resolution <= 96dpi)", dpr(1.0), dpr(2.0)),
      ("(2dppx <= resolution)", dpr(2.0), dpr(1.0)),
      ("(resolution: 37.7952756dpcm)", dpr(1.0), dpr(2.0)),
      ("(resolution: 1dppx)", dpr(1.0), dpr(1.01)),
    ] {
      let sheet = parse_stylesheet(&format!("@media {query} {{ .card {{ width: 100px; }} }}"));
      let Some(media) = sheet.rules[0].media_queries.first() else {
        unreachable!("expected media queries on parsed rule: {query}");
      };

      assert!(media.matches(matching), "{query} should match");
      assert!(!media.matches(non_matching), "{query} should not match");
    }

    let negative = parse_stylesheet("@media (min-resolution: -1dppx) { .card { width: 1px; } }");
    assert!(
      negative.rules[0].media_queries[0].matches(dpr(1.0)),
      "a negative lower bound is always true"
    );

    let max_ratio = parse_stylesheet("@media (max-aspect-ratio: 2/1) { .card { width: 1px; } }");
    assert!(max_ratio.rules[0].media_queries[0].matches(Viewport::new((800, 400))));
    assert!(!max_ratio.rules[0].media_queries[0].matches(Viewport::new((900, 400))));

    let degenerate = parse_stylesheet("@media (aspect-ratio <= 1/0) { .card { width: 100px; } }");
    assert!(
      !degenerate.rules[0].media_queries[0].matches(Viewport::new((800, 400))),
      "a zero divisor is not a ratio"
    );

    let boolean = parse_stylesheet("@media (resolution) { .card { width: 100px; } }");
    assert!(
      boolean.rules[0].media_queries[0].matches(dpr(1.0)),
      "a render always has a resolution"
    );

    for (query, matching, non_matching) in [
      ("(aspect-ratio: 2/1)", (800, 400), (900, 400)),
      ("(aspect-ratio >= 16/9)", (1600, 900), (800, 600)),
      ("(aspect-ratio < 1)", (400, 800), (800, 400)),
      ("(1/1 < aspect-ratio <= 2/1)", (800, 400), (900, 400)),
      ("(aspect-ratio)", (800, 400), (0, 400)),
    ] {
      let sheet = parse_stylesheet(&format!("@media {query} {{ .card {{ width: 100px; }} }}"));
      let Some(media) = sheet.rules[0].media_queries.first() else {
        unreachable!("expected media queries on parsed rule: {query}");
      };

      assert!(
        media.matches(Viewport::new(matching)),
        "{query} at {matching:?}"
      );
      assert!(
        !media.matches(Viewport::new(non_matching)),
        "{query} at {non_matching:?}"
      );
    }
  }

  #[test]
  fn test_parse_media_rule_with_boolean_logic() {
    for (query, matching, non_matching) in [
      (
        "(min-width: 600px) or (min-height: 900px)",
        (400, 1000),
        (400, 400),
      ),
      (
        "((min-width: 600px) or (min-height: 900px))",
        (800, 400),
        (400, 400),
      ),
      ("not (min-width: 600px)", (400, 400), (800, 400)),
      (
        "(not (min-width: 600px)) or (orientation: portrait)",
        (400, 800),
        (800, 400),
      ),
      (
        "(width > 100px) and ((height < 500px) or (orientation: portrait))",
        (800, 400),
        (800, 600),
      ),
      (
        "screen and ((min-width: 600px) or (min-height: 900px))",
        (800, 400),
        (400, 400),
      ),
    ] {
      let sheet = parse_stylesheet(&format!("@media {query} {{ .card {{ width: 100px; }} }}"));
      let Some(media) = sheet.rules[0].media_queries.first() else {
        unreachable!("expected media queries on parsed rule: {query}");
      };

      assert!(
        media.matches(Viewport::new(matching)),
        "{query} at {matching:?}"
      );
      assert!(
        !media.matches(Viewport::new(non_matching)),
        "{query} at {non_matching:?}"
      );
    }
  }

  #[test]
  fn test_parse_media_rule_rejects_a_bare_or_after_a_media_type() {
    let sheet = parse_stylesheet(
      "@media screen and (min-width: 600px) or (min-width: 1px) { .card { width: 100px; } }",
    );
    let Some(media) = sheet.rules[0].media_queries.first() else {
      unreachable!("expected media queries on parsed rule");
    };

    assert!(!media.matches(Viewport::new((800, 400))));
  }

  #[test]
  fn test_parse_media_rule_with_media_types() {
    let screen = Viewport::new((800, 600));
    let print = screen.with_media_target(MediaTarget::Print);

    for (prelude, matches_screen, matches_print) in [
      ("screen", true, false),
      ("print", false, true),
      ("all", true, true),
      ("print and (min-width: 600px)", false, true),
      ("not print", true, false),
      ("only screen", true, false),
    ] {
      let sheet = parse_stylesheet(&format!("@media {prelude} {{ .card {{ width: 100px; }} }}"));
      let Some(media) = sheet.rules[0].media_queries.first() else {
        unreachable!("expected media queries on parsed rule: {prelude}");
      };

      assert_eq!(media.matches(screen), matches_screen, "{prelude} on screen");
      assert_eq!(media.matches(print), matches_print, "{prelude} on print");
    }
  }

  #[test]
  fn test_parse_media_rule_replaces_malformed_queries_with_not_all() {
    for prelude in [
      "(400px < width > 700px)",
      "(unsupported-feature: 10px)",
      "not (unsupported-feature: 10px)",
      "(min-width: 600px) trailing-junk",
      "braille",
    ] {
      let sheet = parse_stylesheet(&format!("@media {prelude} {{ .card {{ width: 100px; }} }}"));
      let Some(media) = sheet.rules[0].media_queries.first() else {
        unreachable!("expected media queries on parsed rule: {prelude}");
      };

      assert!(!media.matches(Viewport::new((800, 600))), "{prelude}");
    }
  }

  #[test]
  fn test_parse_media_rule_keeps_valid_queries_beside_malformed_ones() {
    let sheet = parse_stylesheet(
      r#"
        @media (unsupported-feature: 10px), (min-width: 600px) {
          .card { width: 100px; }
        }
      "#,
    );

    let Some(media) = sheet.rules[0].media_queries.first() else {
      unreachable!("expected media queries on parsed rule");
    };
    assert!(media.matches(Viewport::new((800, 600))));
    assert!(!media.matches(Viewport::new((400, 600))));
  }

  #[test]
  fn test_parse_media_rule_applies_to_keyframes_and_property_rules() {
    let sheet = parse_stylesheet(
      r#"
        @media (min-width: 600px) {
          @keyframes fade {
            from { opacity: 0; }
            to { opacity: 1; }
          }

          @property --box-size {
            syntax: "<length>";
            inherits: false;
            initial-value: 10px;
          }
        }
      "#,
    );

    assert_eq!(sheet.keyframes.len(), 1);
    assert_eq!(sheet.property_rules.len(), 1);
    assert!(
      sheet.keyframes[0]
        .media_queries
        .first()
        .is_some_and(|media| media.matches(Viewport::new((800, 600))))
    );
    assert!(
      sheet.property_rules[0]
        .media_queries
        .first()
        .is_some_and(|media| media.matches(Viewport::new((800, 600))))
    );
  }

  #[test]
  fn test_parse_nested_rule_is_flattened() {
    let sheet = parse_stylesheet(
      r#"
        .card {
          width: 100px;
          .title { height: 20px; }
          & > .icon { width: 12px; }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 3);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(selector_text(&sheet.rules[1]), ":is(.card) .title");
    assert_eq!(selector_text(&sheet.rules[2]), ":is(.card) > .icon");
  }

  #[test]
  fn test_parse_nested_rule_cross_product_for_selector_lists() {
    let sheet = parse_stylesheet(
      r#"
        .card, .panel {
          & .title, & .subtitle { width: 12px; }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(
      selector_text(&sheet.rules[0]),
      ":is(.card, .panel) .title, :is(.card, .panel) .subtitle"
    );
  }

  #[test]
  fn test_parse_nested_rule_uses_is_wrapper_for_multi_parent_lists() {
    let sheet = parse_stylesheet(
      r#"
        .card, .panel {
          & + .item { width: 12px; }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ":is(.card, .panel) + .item");
  }

  #[test]
  fn test_parse_nested_media_and_supports_rules() {
    let sheet = parse_stylesheet(
      r#"
        .card {
          @media (min-width: 600px) {
            @supports (display: grid) {
              width: 100px;
            }
          }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(sheet.rules[0].media_queries.len(), 1);
    assert!(
      sheet.rules[0]
        .media_queries
        .first()
        .is_some_and(|media| media.matches(Viewport::new((800, 600))))
    );
  }

  #[test]
  fn test_parse_multiple_nested_media_queries_accumulate() {
    let sheet = parse_stylesheet(
      r#"
        .card {
          @media (min-width: 600px) {
            @media (orientation: landscape) {
              width: 100px;
            }
          }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(sheet.rules[0].media_queries.len(), 2);
    assert!(sheet.rules[0].media_queries[0].matches(Viewport::new((800, 600))));
    assert!(sheet.rules[0].media_queries[1].matches(Viewport::new((800, 600))));
    assert!(!sheet.rules[0].media_queries[1].matches(Viewport::new((500, 800))));
  }

  #[test]
  fn test_parse_supports_rule_filters_unsupported_declarations() {
    let sheet = parse_stylesheet(
      r#"
        @supports (display: grid) {
          .card { width: 100px; }
        }

        @supports (unknown-prop: nope) {
          .card { height: 20px; }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations)
        .box_data
        .width,
      Length::Px(100.0).into()
    );
  }

  #[test]
  fn test_parse_supports_not_and_or_conditions() {
    let sheet = parse_stylesheet(
      r#"
        @supports (display: grid) and (not (unknown-prop: nope)) {
          .grid { width: 10px; }
        }

        @supports (unknown-prop: nope) or (display: flex) {
          .flex { height: 20px; }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 2);
    assert_eq!(selector_text(&sheet.rules[0]), ".grid");
    assert_eq!(selector_text(&sheet.rules[1]), ".flex");
  }

  #[test]
  fn test_parse_supports_mixed_and_or_requires_parentheses() {
    let sheet = parse_stylesheet_loosy(
      r#"
        @supports (display: grid) and (color: red) or (display: flex) {
          .invalid { width: 10px; }
        }

        .valid { height: 20px; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ".valid");
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations)
        .box_data
        .height,
      Length::Px(20.0).into()
    );
  }

  #[test]
  fn test_parse_property_rule() {
    let sheet = parse_stylesheet(
      r#"
        @property --box-size {
          syntax: "<length>";
          inherits: false;
          initial-value: 10px;
        }
      "#,
    );

    assert_eq!(sheet.property_rules.len(), 1);
    assert_eq!(sheet.property_rules[0].name, "--box-size");
    assert_eq!(sheet.property_rules[0].syntax.to_string(), "<length>");
    assert!(!sheet.property_rules[0].inherits);
    assert_eq!(
      sheet.property_rules[0].initial_value,
      Some("10px".to_owned())
    );
  }

  #[test]
  fn test_parse_property_rule_drops_important_descriptors() {
    let sheet = parse_stylesheet(
      r#"
        @property --box-size {
          syntax: "<length>";
          inherits: false;
          initial-value: 10px;
          initial-value: 20px !important;
        }
      "#,
    );

    assert_eq!(
      sheet.property_rules[0].initial_value,
      Some("10px".to_owned())
    );

    let sheet = parse_stylesheet_loosy(
      r#"
        @property --box-size {
          syntax: "<length>";
          inherits: false !important;
          initial-value: 10px;
        }
      "#,
    );

    assert!(sheet.property_rules.is_empty());
  }

  #[test]
  fn test_parse_property_rule_descriptors_case_insensitively() {
    let sheet = parse_stylesheet(
      r#"
        @property --box-size {
          SYNTAX: "<length>";
          InHeRiTs: false;
          INITIAL-VALUE: 10px;
        }
      "#,
    );

    assert_eq!(sheet.property_rules.len(), 1);
    assert_eq!(sheet.property_rules[0].name, "--box-size");
    assert_eq!(sheet.property_rules[0].syntax.to_string(), "<length>");
    assert!(!sheet.property_rules[0].inherits);
    assert_eq!(
      sheet.property_rules[0].initial_value,
      Some("10px".to_owned())
    );
  }

  #[test]
  fn test_parse_property_rule_needs_an_initial_value_unless_universal() {
    let sheet = parse_stylesheet(
      r#"
        @property --tw-rotate-x {
          syntax: "*";
          inherits: false;
        }
      "#,
    );

    assert_eq!(sheet.property_rules.len(), 1);
    assert_eq!(sheet.property_rules[0].name, "--tw-rotate-x");
    assert_eq!(sheet.property_rules[0].syntax.to_string(), "*");
    assert!(!sheet.property_rules[0].inherits);
    assert_eq!(sheet.property_rules[0].initial_value, None);

    // A typed syntax has nothing to fall back to without one, so the rule goes.
    let sheet = parse_stylesheet_loosy(
      r#"
        @property --box-size {
          syntax: "<length>";
          inherits: false;
        }

        .card { width: 100px; }
      "#,
    );

    assert!(sheet.property_rules.is_empty());
    assert_eq!(sheet.rules.len(), 1);
  }

  #[test]
  fn test_parse_property_rule_supports_extended_syntaxes() {
    let sheet = parse_stylesheet(
      r#"
        @property --accent {
          syntax: "<length> | <color>";
          inherits: false;
          initial-value: red;
        }
        @property --display-state {
          syntax: "none | auto";
          inherits: false;
          initial-value: none;
        }
        @property --fade-duration {
          syntax: "<time>";
          inherits: false;
          initial-value: 150ms;
        }
        @property --move {
          syntax: "<transform-function>";
          inherits: false;
          initial-value: translate(10px, 20px);
        }
        @property --bg {
          syntax: "<image>";
          inherits: false;
          initial-value: linear-gradient(red, blue);
        }
      "#,
    );

    let syntaxes: Vec<_> = sheet
      .property_rules
      .iter()
      .map(|rule| rule.syntax.to_string())
      .collect();

    assert_eq!(
      syntaxes,
      [
        "<length> | <color>",
        "none | auto",
        "<time>",
        "<transform-function>",
        "<image>"
      ]
    );
  }

  #[test]
  fn test_lossy_parse_rejects_invalid_property_rules() {
    for css in [
      r#"
        @property color {
          syntax: "<color>";
          inherits: false;
          initial-value: red;
        }

        .card { width: 100px; }
      "#,
      r#"
        @property --box-size {
          inherits: false;
          initial-value: 10px;
        }

        .card { width: 100px; }
      "#,
      r#"
        @property --accent {
          syntax: "<color>";
          initial-value: red;
        }

        .card { width: 100px; }
      "#,
      r#"
        @property --box-size {
          syntax: "<length>";
          inherits: maybe;
          initial-value: 10px;
        }

        .card { width: 100px; }
      "#,
    ] {
      let sheet = parse_stylesheet_loosy(css);

      assert!(sheet.property_rules.is_empty());
      assert_eq!(sheet.rules.len(), 1);
      assert_eq!(selector_text(&sheet.rules[0]), ".card");
      assert_eq!(
        computed_style_from_declarations(&sheet.rules[0].normal_declarations)
          .box_data
          .width,
        Length::Px(100.0).into()
      );
    }
  }

  #[test]
  fn test_parse_stylesheet_returns_error_for_invalid_property_rule() {
    let result = StyleSheet::parse(
      r#"
        @property --box-size {
          inherits: false;
          initial-value: 10px;
        }
      "#,
    );

    assert!(result.is_err());
  }

  /// A name the syntax grammar does not list, or an initial value that needs the element to
  /// compute, drops the rule, as Chromium drops it.
  #[test]
  fn test_property_rules_chromium_drops() {
    let sheet = parse_stylesheet_loosy(
      r#"
        @property --curve { syntax: "<easing-function>"; inherits: false; initial-value: ease }
        @property --fx { syntax: "<filter-function>"; inherits: false; initial-value: blur(4px) }
        @property --list { syntax: "<transform-list>+"; inherits: false; initial-value: none }
        @property --spaced { syntax: "<length> +"; inherits: false; initial-value: 1px }
        @property --wide { syntax: "inherit"; inherits: false; initial-value: inherit }
        @property --var { syntax: "<length>"; inherits: false; initial-value: var(--fallback) }
        @property --em { syntax: "<length>"; inherits: false; initial-value: 1em }
        @property --vw { syntax: "<length>"; inherits: false; initial-value: 10vw }
        @property --off { syntax: "<length>"; inherits: false; initial-value: red }
        @property --bare { syntax: <length>; inherits: false; initial-value: 1px }
        @property --escaped { syntax: "\3c length\3e"; inherits: false; initial-value: 2px }
        @property --kept { syntax: "<length>"; inherits: false; initial-value: 1in }
      "#,
    );
    let kept: Vec<_> = sheet
      .property_rules
      .iter()
      .map(|rule| (rule.name.as_str(), rule.initial_value.as_deref()))
      .collect();

    assert_eq!(kept, [("--escaped", Some("2px")), ("--kept", Some("1in"))]);
  }

  #[test]
  fn test_parse_layer_rule_without_block() {
    let sheet = parse_stylesheet(
      r#"
        @layer theme, base, components, utilities;
        @layer utilities {
          .card { width: 100px; }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(
      sheet.rules[0].layer.as_ref(),
      Some(&vec![LayerName::Named("utilities".to_owned())])
    );
    assert_eq!(sheet.rules[0].layer_order, Some(3));
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations)
        .box_data
        .width,
      Length::Px(100.0).into()
    );
  }

  #[test]
  fn test_parse_nested_layers_are_transparent() {
    let sheet = parse_stylesheet(
      r#"
        @layer theme {
          @layer components {
            .card { width: 100px; }
          }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(
      sheet.rules[0].layer.as_ref(),
      Some(&vec![
        LayerName::Named("theme".to_owned()),
        LayerName::Named("components".to_owned()),
      ])
    );
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations)
        .box_data
        .width,
      Length::Px(100.0).into()
    );
  }

  #[test]
  fn test_parse_nested_layer_inside_style_rule_preserves_parent_selector() {
    let sheet = parse_stylesheet(
      r#"
        .card {
          @layer theme {
            width: 100px;
            .title { height: 20px; }
          }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 2);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(selector_text(&sheet.rules[1]), ":is(.card) .title");
    assert_eq!(
      sheet.rules[0].layer.as_ref(),
      Some(&vec![LayerName::Named("theme".to_owned())])
    );
    assert_eq!(
      sheet.rules[1].layer.as_ref(),
      Some(&vec![LayerName::Named("theme".to_owned())])
    );
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations)
        .box_data
        .width,
      Length::Px(100.0).into()
    );
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[1].normal_declarations)
        .box_data
        .height,
      Length::Px(20.0).into()
    );
  }

  #[test]
  fn test_parse_anonymous_nested_layer_has_distinct_order() {
    let sheet = parse_stylesheet(
      r#"
        @layer theme {
          .parent { width: 10px; }

          @layer {
            .child { width: 20px; }
          }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 2);
    assert_eq!(
      sheet.rules[0].layer.as_ref(),
      Some(&vec![LayerName::Named("theme".to_owned())])
    );
    assert!(matches!(
      sheet.rules[1].layer.as_deref(),
      Some([LayerName::Named(theme), LayerName::Anonymous(_)]) if theme == "theme"
    ));
    assert_ne!(sheet.rules[0].layer_order, sheet.rules[1].layer_order);
  }

  /// The layer order of each rule in `sheet`, in source order.
  fn rule_layer_orders(sheet: &StyleSheet) -> Vec<Option<usize>> {
    sheet.rules.iter().map(|rule| rule.layer_order).collect()
  }

  /// Each anonymous `@layer` block is a layer of its own, in each sheet of a list too, as Blink's
  /// `CascadeLayer::FindDirectSubLayer` finds no anonymous layer.
  #[test]
  fn each_anonymous_layer_is_its_own_layer() {
    let sheet = parse_stylesheet("@layer { .a { width: 1px } } @layer { .b { width: 2px } }");
    let [first, second] = rule_layer_orders(&sheet)[..] else {
      panic!("two rules");
    };

    assert!(first < second);

    let sheet = StyleSheet::parse_list([
      "@layer { .a { width: 1px } }",
      "@layer { .b { width: 2px } }",
    ])
    .expect("parses");
    let [first, second] = rule_layer_orders(&sheet)[..] else {
      panic!("two rules");
    };

    assert!(first < second);
  }

  /// Layers order as Blink's `ComputeLayerOrder` walks their tree: a layer after its sublayers, and
  /// the sublayers of one layer together.
  #[test]
  fn a_layer_orders_after_its_sublayers() {
    let sheet = parse_stylesheet("@layer a { .x { width: 1px } } @layer a.b { .x { width: 2px } }");
    let [a, a_b] = rule_layer_orders(&sheet)[..] else {
      panic!("two rules");
    };

    assert!(a_b < a);

    let sheet = parse_stylesheet(
      "@layer a.b; @layer c; @layer a.d;
       @layer c { .x { width: 1px } } @layer a.d { .x { width: 2px } } @layer a.b { .x { width: 3px } }",
    );
    let [c, a_d, a_b] = rule_layer_orders(&sheet)[..] else {
      panic!("three rules");
    };

    assert!(a_b < a_d && a_d < c);

    let sheet = parse_stylesheet(".x { @layer b { width: 1px } } @layer c { .x { width: 2px } }");
    let [b, c] = rule_layer_orders(&sheet)[..] else {
      panic!("two rules");
    };

    assert!(b < c);
  }

  /// A layer path deeper than [`MAX_LAYER_DEPTH`] drops its rule, so a long dotted name or deeply
  /// nested blocks neither overflow the stack nor take quadratic time.
  #[test]
  fn a_too_deep_layer_path_drops_its_rule() {
    let name = vec!["a"; 100_000].join(".");
    let sheet = parse_stylesheet_loosy(&format!(
      "@layer {name} {{ .x {{ width: 1px }} }} @layer {name}; .y {{ width: 2px }}"
    ));

    assert_eq!(rule_layer_orders(&sheet), [None]);

    let depth = 10_000;
    let sheet = parse_stylesheet_loosy(&format!(
      "{}.x {{ width: 1px }}{}",
      "@layer a { ".repeat(depth),
      "}".repeat(depth)
    ));

    assert!(sheet.rules.is_empty());

    let at_limit = vec!["a"; MAX_LAYER_DEPTH].join(".");
    let sheet = parse_stylesheet(&format!("@layer {at_limit} {{ .x {{ width: 1px }} }}"));

    assert_eq!(rule_layer_orders(&sheet), [Some(0)]);
  }

  /// An invalid layer name drops its block; it does not fall back to an
  /// anonymous layer.
  #[test]
  fn test_parse_invalid_layer_name_drops_the_block() {
    for prelude in ["inherit", "\"quoted\"", "base.default"] {
      let sheet = parse_stylesheet_loosy(&format!(
        r#"
          @layer {prelude} {{ .card {{ width: 50px; }} }}
          .card {{ width: 100px; }}
        "#
      ));

      assert_eq!(sheet.rules.len(), 1);
      assert_eq!(sheet.rules[0].layer, None);
    }
  }

  #[test]
  fn test_parse_layer_block_rejects_multiple_names() {
    let sheet = parse_stylesheet_loosy(
      r#"
        @layer theme, components {
          .invalid { width: 10px; }
        }

        .valid { height: 20px; }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ".valid");
    assert_eq!(sheet.rules[0].layer, None);
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations)
        .box_data
        .height,
      Length::Px(20.0).into()
    );
  }

  #[test]
  fn test_parse_nested_rules_preserves_source_order() {
    let sheet = parse_stylesheet(
      r#"
        .card {
          width: 100px;
          & .title { color: red; }
          height: 20px;
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 3);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations)
        .box_data
        .width,
      Length::Px(100.0).into()
    );
    assert_eq!(selector_text(&sheet.rules[1]), ":is(.card) .title");
    assert_eq!(selector_text(&sheet.rules[2]), ".card");
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[2].normal_declarations)
        .box_data
        .height,
      Length::Px(20.0).into()
    );
  }

  #[test]
  fn test_nested_unsupported_supports_rule_is_discarded() {
    let sheet = parse_stylesheet(
      r#"
        .card {
          width: 100px;
          @supports (unknown-prop: nope) {
            height: 20px;
            & .title { color: red; }
          }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");

    let computed = computed_style_from_declarations(&sheet.rules[0].normal_declarations);
    assert_eq!(computed.box_data.width, Length::Px(100.0).into());
    assert_eq!(computed.box_data.height, Length::Auto.into());
  }

  #[test]
  fn test_nested_keyframes_rule_is_rejected() {
    let sheet = parse_stylesheet_loosy(
      r#"
        .card {
          width: 100px;
          @keyframes pulse {
            from { opacity: 0; }
            to { opacity: 1; }
          }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(sheet.keyframes.len(), 0);
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations)
        .box_data
        .width,
      Length::Px(100.0).into()
    );
  }

  #[test]
  fn test_nested_property_rule_is_rejected() {
    let sheet = parse_stylesheet_loosy(
      r#"
        .card {
          width: 100px;
          @property --box-size {
            syntax: "<length>";
            inherits: false;
            initial-value: 10px;
          }
        }
      "#,
    );

    assert_eq!(sheet.rules.len(), 1);
    assert!(sheet.property_rules.is_empty());
    assert_eq!(selector_text(&sheet.rules[0]), ".card");
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[0].normal_declarations)
        .box_data
        .width,
      Length::Px(100.0).into()
    );
  }

  #[test]
  fn test_lossy_parse_rejects_unknown_rules_and_keeps_valid_siblings() {
    assert_lossy_parse_keeps_single_valid_rule(
      r#"
        @unknown something {
          .card { width: 10px; }
        }

        .card { width: 100px; }
      "#,
    );
  }

  #[test]
  fn test_parse_skips_unknown_at_rules() {
    let sheets = [
      r#"@font-face { font-family: Geist; src: url(geist.woff2); } .card { width: 100px; }"#,
      r#"@charset "utf-8"; .card { width: 100px; }"#,
      r#"@unknown something { .card { width: 10px; } } .card { width: 100px; }"#,
      r#".card { @unknown something { width: 10px; } width: 100px; }"#,
      r#".card { @unknown something; width: 100px; }"#,
    ];

    for css in sheets {
      let sheet = parse_stylesheet(css);
      assert_eq!(sheet.rules.len(), 1, "{css}");
      assert_eq!(
        computed_style_from_declarations(&sheet.rules[0].normal_declarations)
          .box_data
          .width,
        Length::Px(100.0).into(),
        "{css}"
      );
    }
  }

  #[test]
  fn test_theme_block_reads_as_root_rule() {
    let sheet = parse_stylesheet("@theme { --color-brand: #ff0000; }");
    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ":root");
    assert_eq!(
      sheet.rules[0].normal_declarations.declarations.as_slice(),
      &[StyleDeclaration::CustomProperty(
        "--color-brand".into(),
        "#ff0000".into()
      )]
    );
  }

  /// A reference token lands on `:root`, since utilities here resolve `var()` through the cascade.
  #[test]
  fn test_theme_reference_still_emits_root() {
    let sheet = parse_stylesheet("@theme reference { --color-brand: #ff0000; }");
    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ":root");
  }

  /// A custom property swallows `!important` into its value, so only a
  /// non-custom declaration exercises the split.
  #[test]
  fn test_theme_splits_important_declarations() {
    let sheet = parse_stylesheet("@theme { --x: 1px; width: 10px !important; }");
    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(sheet.rules[0].normal_declarations.declarations.len(), 1);
    assert_eq!(sheet.rules[0].important_declarations.declarations.len(), 1);
  }

  #[test]
  fn test_theme_compiler_modifiers_are_accepted() {
    let sheet = parse_stylesheet("@theme default static inline { --x: 1px; }");
    assert_eq!(sheet.rules.len(), 1);
  }

  #[test]
  fn test_theme_prefix_modifier_is_rejected() {
    assert_lossy_parse_keeps_single_valid_rule(
      r#"
        @theme prefix(tw) { --x: 1px; }

        .card { width: 100px; }
      "#,
    );
  }

  #[test]
  fn test_keyframes_inside_theme() {
    let sheet = parse_stylesheet(
      r#"
        @theme {
          --animate-wobble: wobble 1s linear infinite;
          @keyframes wobble {
            to { width: 10px; }
          }
        }
      "#,
    );
    assert_eq!(sheet.keyframes.len(), 1);
    assert_eq!(sheet.keyframes[0].name, "wobble");
    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(selector_text(&sheet.rules[0]), ":root");
  }

  #[test]
  fn test_breakpoint_variables_are_collected() {
    let sheet = parse_stylesheet(
      r#"
        @theme { --breakpoint-3xl: 120rem; }
        :root { --breakpoint-md: 30rem; }
        @media (min-width: 5000px) { :root { --breakpoint-lg: 10rem; } }
        .card { --breakpoint-xl: 10rem; }
      "#,
    );

    assert_eq!(sheet.breakpoints.get("3xl"), Some(&Length::Rem(120.0)));
    assert_eq!(sheet.breakpoints.get("md"), Some(&Length::Rem(30.0)));
    assert_eq!(sheet.breakpoints.get("lg"), None);
    assert_eq!(sheet.breakpoints.get("xl"), None);
  }

  #[test]
  fn test_breakpoint_takes_the_winning_cascade_layer() {
    let sheet = parse_stylesheet(
      r#"
        @layer base, theme;
        :root { --breakpoint-md: 30rem; }
        @layer theme { :root { --breakpoint-md: 50rem; --breakpoint-lg: 70rem; } }
        @layer base { :root { --breakpoint-lg: 60rem; } }
      "#,
    );

    assert_eq!(sheet.breakpoints.get("md"), Some(&Length::Rem(30.0)));
    assert_eq!(sheet.breakpoints.get("lg"), Some(&Length::Rem(70.0)));
  }

  /// A width `Breakpoint::matches` cannot resolve stays out of the overrides,
  /// so the variant keeps its built-in width instead of never matching.
  #[test]
  fn test_breakpoint_with_unresolvable_unit_is_ignored() {
    let sheet = parse_stylesheet(":root { --breakpoint-md: 48em; }");

    assert_eq!(sheet.breakpoints.get("md"), None);
  }

  #[test]
  fn test_apply_expands_utilities_in_place() {
    let sheet = parse_stylesheet(".card { width: 100px; @apply mt-4; height: 50px; }");

    assert_eq!(sheet.rules.len(), 3);
    for rule in sheet.rules() {
      assert_eq!(selector_text(rule), ".card");
    }
    assert_eq!(
      computed_style_from_declarations(&sheet.rules[1].normal_declarations)
        .box_data
        .margin_top,
      Length::Rem(1.0)
    );
  }

  #[test]
  fn test_apply_treats_comments_as_separators() {
    let sheet = parse_stylesheet(".card { @apply mt-4 /* note */ p-2; }");

    assert_eq!(sheet.rules.len(), 1);
    let computed = computed_style_from_declarations(&sheet.rules[0].normal_declarations);
    assert_eq!(computed.box_data.margin_top, Length::Rem(1.0));
    assert_eq!(computed.box_data.padding_top, Length::Rem(0.5));
  }

  #[test]
  fn test_apply_keeps_the_important_suffix() {
    let sheet = parse_stylesheet(".card { @apply mt-4!; }");

    assert_eq!(sheet.rules.len(), 1);
    assert!(sheet.rules[0].normal_declarations.declarations.is_empty());
    assert!(
      !sheet.rules[0]
        .important_declarations
        .declarations
        .is_empty()
    );
  }

  #[test]
  fn test_apply_rejects_variants_and_unknown_utilities() {
    for css in [
      ".card { @apply md:mt-4; } .card { width: 100px; }",
      ".card { @apply not-a-utility-#%; } .card { width: 100px; }",
    ] {
      assert_lossy_parse_keeps_single_valid_rule(css);
    }
  }

  fn preflight_rule_count(sheet: &StyleSheet) -> usize {
    sheet
      .rules
      .iter()
      .filter(|rule| {
        rule.layer.as_ref().is_some_and(|layer| {
          layer
            .first()
            .is_some_and(|name| matches!(name, LayerName::Named(named) if named == "base"))
        })
      })
      .count()
  }

  /// Pins every Preflight declaration. A property takumi stops supporting is
  /// dropped silently by both parse modes, so only the count catches it.
  #[test]
  fn test_preflight_css_keeps_every_declaration() {
    let sheet = StyleSheet::parse(PREFLIGHT_CSS).unwrap();
    let declarations: usize = sheet
      .rules
      .iter()
      .map(|rule| {
        rule.normal_declarations.declarations.len() + rule.important_declarations.declarations.len()
      })
      .sum();

    assert_eq!(sheet.rules.len(), 16);
    assert_eq!(declarations, 56);
  }

  #[test]
  fn test_tailwind_import_turns_preflight_on() {
    let imports = [
      r#"@import "tailwindcss";"#,
      r#"@import "tailwindcss" source(none);"#,
      r#"@import "tailwindcss" important;"#,
      r#"@import "tailwindcss/index.css";"#,
      r#"@import "tailwindcss/preflight.css" layer(base);"#,
      r#"@import "tailwindcss/preflight";"#,
    ];

    for import in imports {
      let sheet = parse_stylesheet(&format!("{import} .card {{ width: 100px; }}"));
      assert!(preflight_rule_count(&sheet) > 0, "{import}");
      assert_eq!(
        sheet.rules.last().map(selector_text),
        Some(".card".into()),
        "{import}"
      );
    }
  }

  /// Tailwind's documented way to leave Preflight out imports only these two.
  #[test]
  fn test_theme_and_utilities_imports_leave_preflight_off() {
    let sheet = parse_stylesheet(
      r#"@layer theme, base, components, utilities;
      @import "tailwindcss/theme.css" layer(theme);
      @import "tailwindcss/utilities.css" layer(utilities);
      .card { width: 100px; }"#,
    );

    assert_eq!(preflight_rule_count(&sheet), 0);
    assert_eq!(sheet.rules.len(), 1);
  }

  /// `@import` is only valid at the top of a stylesheet; a nested one must
  /// not switch Preflight on past its container's gate.
  #[test]
  fn test_nested_tailwind_import_is_rejected() {
    let nested = [
      r#"@media (min-width: 1px) { @import "tailwindcss"; }"#,
      r#"@layer base { @import "tailwindcss"; }"#,
      r#"@supports (display: flex) { @import "tailwindcss"; }"#,
    ];

    for css in nested {
      assert!(StyleSheet::parse(css).is_err(), "{css}");

      let sheet = parse_stylesheet_loosy(css);
      assert_eq!(preflight_rule_count(&sheet), 0, "{css}");
    }
  }

  #[test]
  fn test_other_imports_are_dropped() {
    for import in [
      r#"@import "./app.css";"#,
      r#"@import "TAILWINDCSS";"#,
      r#"@import "tailwindcss/plugin";"#,
    ] {
      let sheet = parse_stylesheet_loosy(&format!("{import} .card {{ width: 100px; }}"));
      assert_eq!(preflight_rule_count(&sheet), 0, "{import}");
      assert_eq!(sheet.rules.len(), 1, "{import}");
    }
  }

  #[test]
  fn test_theme_inside_media_is_gated() {
    let sheet = parse_stylesheet("@media (min-width: 5000px) { @theme { --x: red; } }");
    assert_eq!(sheet.rules.len(), 1);
    assert_eq!(sheet.rules[0].media_queries.len(), 1);
  }
}
