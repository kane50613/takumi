mod animation;
mod calc;
mod css_input;
mod math;
mod media_query;
pub(crate) mod properties;
pub(crate) mod selector;
mod sizing;
mod stylesheets;
mod supports;
/// Tailwind utility-class parsing.
pub mod tw;

pub use animation::apply_stylesheet_animations;
pub use animation::{KeyframeRule, KeyframesRule};
pub(crate) use calc::CalcArena;
pub(crate) use calc::parse_calc_number_expression;
pub(crate) use css_input::{CssInput, CssNumber, CssUnexpected, CssValueSeed};
pub(crate) use math::lerp;
pub use math::{fast_div_255, fast_div_255_u32};
pub(crate) use properties::unexpected_token;
pub use properties::*;
// Selector matching internals (CssRule, SelectorImpl, Ident, …) stay crate-private
// under `selector::` for the renderer's `layout::matching`; only the stylesheet
// entry point and `@property` rule type are part of the public surface.
pub use selector::{MediaQueryList, PropertyRule, StyleSheet};
pub use sizing::SizingContext;
pub use stylesheets::*;
