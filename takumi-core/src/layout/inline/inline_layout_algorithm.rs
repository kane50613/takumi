//! Blink's [`InlineLayoutAlgorithm`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/layout/inline/inline_layout_algorithm.cc)
//! and the line boxes it builds, without ruby, initial letters, SVG text, `text-combine-upright`,
//! `text-box-trim`, block fragmentation, shapes, or margin collapsing.

use taffy::Clear;

use super::{
  inline_break_token::InlineBreakToken,
  inline_child_layout_context::InlineChildLayoutContext,
  inline_item::InlineItemType,
  inline_item_result::LayoutResult,
  inline_node::InlineNode,
  inline_node_data::InlineNodeData,
  justification_utils::apply_justification,
  leading_floats::LeadingFloats,
  line_breaker::{AtomicInlineLayout, LineBreaker, LineBreakerMode},
  line_info::LineInfo,
  line_truncator::LineTruncator,
  line_widths::LineWidths,
  logical_line_builder::LogicalLineBuilder,
  logical_line_item::{LineFragment, LogicalLineItems},
  score_line_breaker::{ScoreLineBreakContext, ScoreLineBreaker},
  text_fit_utils::{LineFitter, ParagraphScale},
};
use crate::{
  fonts::font_height::FontHeight,
  layout::{
    bfc_rect::BfcOffset,
    constraint_space::ConstraintSpace,
    exclusions::{
      exclusion_space::ExclusionSpace,
      layout_opportunity::{LayoutOpportunity, LineLayoutOpportunity},
    },
    floats_utils::{PositionedFloat, UnpositionedFloat, adjust_to_clearance, position_float},
    logical_rect::LogicalOffset,
    margin_strut::MarginStrut,
    tree::RenderNode,
  },
  layout_unit::LayoutUnit,
  style::{
    BlockEllipsis, Direction as TextDirection, TextAlign, TextFitMode, TextFitTarget, TextOverflow,
    TextWrapStyle,
  },
};

/// Blink's `LineBoxFragmentBuilder` and the line's `LogicalLineContainer`: a line box, as
/// `InlineLayoutAlgorithm` returns it.
pub(crate) struct LineBoxFragment<'a, 'c> {
  /// The items, from the line box's line-left and top.
  pub(crate) items: LogicalLineItems<'a, 'c>,
  /// The line box's ascent and descent; empty for an empty line.
  pub(crate) metrics: FontHeight,
  /// The line-left edge, after `text-align` and `text-indent`.
  pub(crate) bfc_line_offset: LayoutUnit,
  /// Blink's `BfcBlockOffset`, none for an empty line.
  pub(crate) bfc_block_offset: Option<LayoutUnit>,
  /// Blink's `LineBoxBfcBlockOffset`: the line box's top, below any floats it moved under.
  pub(crate) line_box_bfc_block_offset: Option<LayoutUnit>,
  pub(crate) inline_size: LayoutUnit,
  /// The width of the spaces hanging past the line's end.
  pub(crate) hang_inline_size: LayoutUnit,
  pub(crate) block_size: LayoutUnit,
  pub(crate) base_direction: TextDirection,
  pub(crate) break_token: Option<InlineBreakToken<'c>>,
  pub(crate) is_empty_line_box: bool,
  /// Blink's `EndMarginStrut`: the margins after the line, which collapse with the next ones.
  pub(crate) end_margin_strut: MarginStrut,
  /// Where the line starts, below the margins before it: an empty line's out-of-flow boxes sit
  /// there, and a block-in-inline that collapses through sits there as if it had a bottom border.
  pub(crate) static_bfc_block_offset: LayoutUnit,
  /// Blink's `IsHiddenDueToLayout`: the line is past the `line-clamp` point.
  pub(crate) is_hidden_due_to_layout: bool,
  /// The space a `<br clear>` adds after the line.
  pub(crate) clearance_after_line: LayoutUnit,
  /// The floats after the line.
  pub(crate) exclusion_space: ExclusionSpace,
  /// Blink's `LinesUntilClamp`, after this line.
  pub(crate) lines_until_clamp: Option<i32>,
  /// Blink's `TextFitScale` of the line container.
  pub(crate) text_fit_scale: f32,
}

impl LineBoxFragment<'_, '_> {
  fn new(exclusion_space: ExclusionSpace) -> Self {
    Self {
      items: LogicalLineItems::default(),
      metrics: FontHeight::empty(),
      bfc_line_offset: LayoutUnit::ZERO,
      bfc_block_offset: None,
      line_box_bfc_block_offset: None,
      inline_size: LayoutUnit::ZERO,
      hang_inline_size: LayoutUnit::ZERO,
      block_size: LayoutUnit::ZERO,
      base_direction: TextDirection::Ltr,
      break_token: None,
      is_empty_line_box: false,
      end_margin_strut: MarginStrut::default(),
      static_bfc_block_offset: LayoutUnit::ZERO,
      is_hidden_due_to_layout: false,
      clearance_after_line: LayoutUnit::ZERO,
      exclusion_space,
      lines_until_clamp: None,
      text_fit_scale: 1.0,
    }
  }

  /// The baseline below the line's top, as Blink's `PropagateBaselineFromLineBox` takes it: a
  /// block-in-inline's own, which it may not have.
  pub(crate) fn baseline(&self) -> Option<LayoutUnit> {
    match self.block_in_inline_layout_result() {
      Some(layout_result) => layout_result.baseline,
      None => Some(self.metrics.ascent),
    }
  }

  /// Blink's `LogicalLineItems::BlockInInlineLayoutResult`.
  pub(crate) fn block_in_inline_layout_result(&self) -> Option<&LayoutResult> {
    self
      .items
      .children
      .iter()
      .find_map(|child| match &child.layout_result {
        Some(LineFragment::BlockInInline { layout_result, .. }) => Some(layout_result),
        _ => None,
      })
  }

  /// Blink's `LineHeight`.
  pub(crate) fn line_height(&self) -> LayoutUnit {
    if self.metrics.is_empty() {
      return LayoutUnit::ZERO;
    }

    self.metrics.line_height().clamp_negative_to_zero()
  }

  /// The baseline's BFC block offset, none for an empty line or a block-in-inline without one.
  pub(crate) fn bfc_baseline(&self) -> Option<LayoutUnit> {
    self
      .line_box_bfc_block_offset
      .zip(self.baseline())
      .map(|(line_top, baseline)| line_top + baseline)
  }

  /// Moves the line box, with the floats placed on it, `delta` toward the block end.
  pub(crate) fn move_in_block_direction(&mut self, delta: LayoutUnit) {
    for offset in [
      &mut self.bfc_block_offset,
      &mut self.line_box_bfc_block_offset,
    ]
    .into_iter()
    .flatten()
    {
      *offset += delta;
    }

    for child in &mut self.items.children {
      if let Some(LineFragment::Float {
        positioned_float, ..
      }) = &mut child.layout_result
      {
        positioned_float.bfc_offset.block_offset += delta;
      }
    }
  }
}

impl TextAlign {
  /// Blink's `LineOffsetForTextAlign`: how far `space_left` moves a line. A line wider than its box
  /// spills out on its end side.
  pub(crate) fn line_offset_for_text_align(
    self,
    direction: TextDirection,
    space_left: LayoutUnit,
  ) -> LayoutUnit {
    let is_ltr = direction == TextDirection::Ltr;
    let text_align = match self {
      Self::Justify => Self::Start,
      text_align => text_align,
    };

    match text_align.resolve(direction) {
      Self::Left if is_ltr => LayoutUnit::ZERO,
      Self::Left => space_left.clamp_positive_to_zero(),
      // In RTL, trailing spaces sit on the line's left.
      Self::Right if !is_ltr => space_left,
      Self::Right => space_left.clamp_negative_to_zero(),
      _ if is_ltr || space_left > LayoutUnit::ZERO => (space_left / 2).clamp_negative_to_zero(),
      _ => space_left,
    }
  }
}

/// Blink's `LineBreakStrategy`: the greedy line breaker, `text-wrap: balance`, or `pretty`.
#[derive(Debug, Clone, Copy, Default)]
struct LineBreakStrategy {
  /// `balance` starts on the paragraph's first line.
  initiate_balancing: bool,
  use_score_line_break: bool,
}

impl LineBreakStrategy {
  fn new(
    context: &InlineChildLayoutContext<'_, '_>,
    text_wrap: TextWrapStyle,
    break_token: Option<InlineBreakToken<'_>>,
  ) -> Self {
    let score_context = context.score_line_break_context.as_ref();

    match text_wrap {
      TextWrapStyle::Balance => {
        let initiate_balancing = break_token.is_none();

        Self {
          initiate_balancing,
          use_score_line_break: initiate_balancing && score_context.is_some(),
        }
      }
      TextWrapStyle::Pretty => Self {
        initiate_balancing: false,
        use_score_line_break: score_context.is_some_and(ScoreLineBreakContext::is_active),
      },
      TextWrapStyle::Auto => Self::default(),
    }
  }

  /// Blink's `NeedsToPrepare`.
  fn needs_to_prepare(self) -> bool {
    self.initiate_balancing || self.use_score_line_break
  }
}

/// Blink's `LineClampState`: how a line shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineClampState {
  Show,
  Hide,
  LineClampEllipsis,
  TextOverflowEllipsis,
}

/// Blink's `InlineLayoutAlgorithm`: lays out one line of an inline formatting context.
pub(crate) struct InlineLayoutAlgorithm<'s, 'a, 'c> {
  node: InlineNode<'c>,
  items_data: &'a InlineNodeData<'c>,
  constraint_space: &'s ConstraintSpace,
  /// Blink's `ConstraintSpace::GetExclusionSpace`: the floats before the line.
  exclusion_space: &'s ExclusionSpace,
  break_token: Option<InlineBreakToken<'c>>,
  context: &'s mut InlineChildLayoutContext<'a, 'c>,
  atomic_inline: &'s mut dyn AtomicInlineLayout<'c>,
  container_builder: LineBoxFragment<'a, 'c>,
  apply_text_fit: bool,
}

impl<'s, 'a, 'c> InlineLayoutAlgorithm<'s, 'a, 'c> {
  pub(crate) fn new(
    node: InlineNode<'c>,
    items_data: &'a InlineNodeData<'c>,
    constraint_space: &'s ConstraintSpace,
    exclusion_space: &'s ExclusionSpace,
    break_token: Option<InlineBreakToken<'c>>,
    context: &'s mut InlineChildLayoutContext<'a, 'c>,
    atomic_inline: &'s mut dyn AtomicInlineLayout<'c>,
  ) -> Self {
    Self {
      node,
      items_data,
      constraint_space,
      exclusion_space,
      break_token,
      context,
      atomic_inline,
      container_builder: LineBoxFragment::new(exclusion_space.clone()),
      apply_text_fit: node.should_apply_text_fit(items_data),
    }
  }

  /// Blink's `Layout`: the line in the first layout opportunity it fits.
  pub(crate) fn layout(mut self) -> LineBoxFragment<'a, 'c> {
    let space = self.constraint_space;
    let mut initial_exclusion_space = self.exclusion_space.clone();
    let mut leading_floats = LeadingFloats::default();

    // The layout opportunities count the floats ahead of any content.
    self.position_leading_floats(&mut initial_exclusion_space, &mut leading_floats);

    let bfc_block_offset = space.bfc_offset.block_offset;
    let opportunities = initial_exclusion_space.all_layout_opportunities(
      BfcOffset::new(space.bfc_offset.line_offset, bfc_block_offset),
      space.inline_size(),
      space.direction,
    );
    let mut lines_until_clamp = space.lines_until_clamp;
    let line_break_strategy = LineBreakStrategy::new(
      self.context,
      self.node.block.context.style.inherited_data.text_wrap_style,
      self.break_token,
    );
    let mut line_box = LogicalLineItems::default();
    let mut line_block_size = LayoutUnit::ZERO;
    let mut block_delta = LayoutUnit::ZERO;
    let mut opportunities_index = 0;

    while opportunities_index < opportunities.len() {
      let opportunity = opportunities[opportunities_index];

      self.container_builder = LineBoxFragment::new(initial_exclusion_space.clone());

      let line_opportunity = opportunity.compute_line_layout_opportunity(
        space.bfc_offset.line_offset,
        space.inline_size(),
        space.direction,
        line_block_size,
        block_delta,
      );
      if line_break_strategy.needs_to_prepare() {
        self.prepare_line_break_strategy(
          line_break_strategy,
          &opportunities[opportunities_index..],
          &line_opportunity,
          &leading_floats,
        );
      }

      let mut line_info = LineInfo::new(self.items_data, &self.node.block.context);
      let mut line_breaker = LineBreaker::new(
        self.node,
        self.items_data,
        LineBreakerMode::Content,
        space.clone(),
        line_opportunity,
        &leading_floats,
        self.break_token,
        &mut *self.atomic_inline,
        &mut self.container_builder.exclusion_space,
      );

      // Blink's `LineBreakStrategy::SetupLineBreaker`.
      if let Some(balanced_available_width) = self.context.balanced_available_width {
        line_breaker.override_available_width(balanced_available_width);
      } else if let Some(break_point) = self
        .context
        .score_line_break_context
        .as_ref()
        .and_then(ScoreLineBreakContext::current_line_break_point)
      {
        line_breaker.set_break_at(*break_point);
      }

      line_breaker.next_line(&mut line_info);
      drop(line_breaker);

      // An empty line has no block offset yet.
      if !line_info.is_empty_line {
        self.container_builder.bfc_block_offset = Some(bfc_block_offset);
        self.container_builder.line_box_bfc_block_offset = Some(line_info.bfc_offset.block_offset);
      }

      // A line too wide for an opportunity beside floats tries the next one.
      if line_info.has_overflow
        && !line_opportunity.is_equal_to_available_float_inline_size(space.inline_size())
        && self
          .node
          .block
          .context
          .style
          .inherited_data
          .text_wrap_mode
          .should_wrap_line()
        && opportunities_index + 1 != opportunities.len()
      {
        block_delta = LayoutUnit::ZERO;
        line_block_size = LayoutUnit::ZERO;
        opportunities_index += 1;

        continue;
      }

      let mut should_scale_line_height = false;

      // The measuring layout of a consistent `text-fit` fits nothing; `measure_per_block_scale`
      // reads its lines.
      if self.apply_text_fit && !self.context.is_measuring_scale {
        let scale = self.context.measured_scale;

        if scale.scale == 1.0 {
          should_scale_line_height =
            LineFitter::new(self.node, &mut line_info).measure_and_fit_line();
        } else {
          LineFitter::new(self.node, &mut line_info)
            .fit_line(scale.scale, Some(scale.additional_paint_time_scale));
          should_scale_line_height = true;
        }
      }

      self.prepare_box_states(&line_info, should_scale_line_height);
      self.create_line(
        &line_opportunity,
        &mut line_info,
        should_scale_line_height,
        &mut line_box,
      );

      // A line taller than the opportunity tries the next one.
      let total_block_size = self.container_builder.line_height();

      if total_block_size + block_delta > opportunity.rect.block_size() {
        block_delta = LayoutUnit::ZERO;
        line_block_size = LayoutUnit::ZERO;
        opportunities_index += 1;

        continue;
      }

      self.container_builder.break_token = line_info.break_token;
      self.container_builder.base_direction = line_info.base_direction;

      if line_info.is_empty_line {
        self.container_builder.is_empty_line_box = true;
      } else {
        self.add_any_clearance_after_line(&line_info);
        self.container_builder.block_size = self.container_builder.line_height();
        lines_until_clamp = lines_until_clamp.map(|lines| lines - 1);
      }

      // Blink's `LineBreakStrategy::DidCreateLine`.
      if let Some(context) = &mut self.context.score_line_break_context {
        context.did_create_line(line_info.is_end_paragraph());
      }

      break;
    }

    self.container_builder.items = line_box;
    self.container_builder.lines_until_clamp = lines_until_clamp;
    self.container_builder
  }

  /// Blink's `LineBreakStrategy::Prepare`: the break points or width that balance or optimize the
  /// paragraph's lines.
  fn prepare_line_break_strategy(
    &mut self,
    strategy: LineBreakStrategy,
    opportunities: &[LayoutOpportunity],
    line_opportunity: &LineLayoutOpportunity,
    leading_floats: &LeadingFloats,
  ) {
    let break_token = if strategy.initiate_balancing {
      None
    } else {
      self.break_token
    };
    let Some(score_context) = self.context.score_line_break_context.as_mut() else {
      if strategy.initiate_balancing {
        self.balance_by_bisection(opportunities, line_opportunity);
      }

      return;
    };

    if strategy.use_score_line_break
      && score_context.is_active()
      && let Some(line_widths) =
        LineWidths::new(self.node, self.items_data, opportunities, break_token)
    {
      let mut score_line_breaker = ScoreLineBreaker::new(
        self.node,
        self.items_data,
        self.constraint_space,
        line_widths,
        break_token,
        &mut self.container_builder.exclusion_space,
        &mut *self.atomic_inline,
      );

      if strategy.initiate_balancing {
        score_line_breaker.balance_break_points(leading_floats, score_context);
      } else {
        score_line_breaker.optimal_break_points(leading_floats, score_context);
      }

      if !score_context.line_break_points.is_empty() {
        return;
      }
    }

    // The bisection when the scores do not apply.
    if strategy.initiate_balancing {
      self.balance_by_bisection(opportunities, line_opportunity);
    }
  }

  /// Blink's bisection in `LineBreakStrategy::Balance`, for one opportunity of positive width.
  fn balance_by_bisection(
    &mut self,
    opportunities: &[LayoutOpportunity],
    line_opportunity: &LineLayoutOpportunity,
  ) {
    if opportunities.len() != 1 || line_opportunity.available_inline_size() <= LayoutUnit::ZERO {
      return;
    }

    self.context.balanced_available_width = self.node.attempt_paragraph_balancing(
      self.items_data,
      self.constraint_space,
      line_opportunity,
      &mut *self.atomic_inline,
    );
  }

  /// Blink's `PrepareBoxStates`: the boxes open at the line's start, kept from the last line when
  /// it ended there.
  fn prepare_box_states(&mut self, line_info: &LineInfo<'a, 'c>, should_scale_line_height: bool) {
    let Some(break_token) = self.break_token else {
      self.context.reset_box_states();

      return;
    };
    let start_item_index = break_token.start.item_index;

    // A fitted line's scale changes the boxes' metrics.
    if !break_token.is_line_clamp_displaced_line
      && !self.apply_text_fit
      && self.context.has_box_states_for_item_index(start_item_index)
    {
      return;
    }

    LogicalLineBuilder::new(
      self.constraint_space,
      self.context.reset_box_states(),
      should_scale_line_height,
    )
    .rebuild_box_states(line_info, 0, start_item_index);
  }

  /// Blink's `CreateLine`.
  fn create_line(
    &mut self,
    opportunity: &LineLayoutOpportunity,
    line_info: &mut LineInfo<'a, 'c>,
    should_scale_line_height: bool,
    line_box: &mut LogicalLineItems<'a, 'c>,
  ) {
    let space = self.constraint_space;

    // Justification changes the items' sizes, which the inline static positions need.
    let line_offset_for_text_align = line_info.apply_text_align();

    line_box.children.clear();

    let mut line_builder =
      LogicalLineBuilder::new(space, self.context.box_states(), should_scale_line_height);

    line_builder.create_line(line_info, line_box);

    let has_out_of_flow_positioned_items = line_builder.has_out_of_flow_positioned_items;
    let has_floating_items = line_builder.has_floating_items;
    let has_relative_positioned_items = line_builder.has_relative_positioned_items;
    let hang_width = line_info.hang_width();
    let position = line_info.adjust_line_offset_for_hanging();
    let box_states = self.context.box_states();
    let mut inline_size =
      box_states.compute_inline_positions(line_box, position, line_info.is_block_in_inline);

    if hang_width != LayoutUnit::ZERO {
      // A line moved by its hanging width already has the right size.
      if position == LayoutUnit::ZERO {
        inline_size -= hang_width;
      }

      self.container_builder.hang_inline_size = hang_width;
    }

    // An empty line that still shows takes the strut's height.
    if line_info.has_line_even_if_empty {
      box_states.line_box_state().ensure_text_metrics(None);
    }

    let line_box_metrics = box_states.line_box_state().metrics;

    let line_clamp_state = self.line_clamp_state(line_info);

    // Approximate: Blink also truncates the layout that measures a consistent `text-fit`, so the
    // ellipsized text it then measures fits no line. takumi fits the text first, and only a line
    // that still overflows at its fitted size takes an ellipsis.
    if matches!(
      line_clamp_state,
      LineClampState::LineClampEllipsis | LineClampState::TextOverflowEllipsis
    ) && !self.context.is_measuring_scale
    {
      let is_ellipsis_caused_by_line_clamp = line_clamp_state == LineClampState::LineClampEllipsis;

      inline_size = LineTruncator::new(line_info, is_ellipsis_caused_by_line_clamp).truncate_line(
        inline_size,
        line_box,
        self.context.box_states(),
      );
    }

    if line_clamp_state == LineClampState::Hide {
      self.container_builder.is_hidden_due_to_layout = true;

      for child in &mut line_box.children {
        child.is_hidden_due_to_layout = true;
      }
    }

    // Negative margins can move the position before zero, but not shrink the size below it.
    inline_size = inline_size.clamp_negative_to_zero();

    if line_info.is_block_in_inline {
      self.container_builder.bfc_line_offset = space.bfc_offset.line_offset;
    } else {
      // Other `text-align` values than `justify` move the line box as a whole.
      let mut bfc_line_offset = line_info.bfc_offset.line_offset + line_offset_for_text_align;

      if line_info.base_direction == TextDirection::Ltr {
        bfc_line_offset += line_info.text_indent;
      }

      self.container_builder.bfc_line_offset = bfc_line_offset;
    }

    if has_out_of_flow_positioned_items {
      self.place_out_of_flow_objects(line_info, &line_box_metrics, line_box);
    }

    if has_floating_items {
      self.place_floating_objects(&line_box_metrics, opportunity, line_info, line_box);
    }

    // `position: relative` moves atomic inlines and floats by themselves, and boxes with what
    // they hold.
    if has_relative_positioned_items {
      line_box.place_relative_positioned_items(space);
    }

    let box_states = self.context.box_states();

    box_states.apply_relative_positioning(space, line_box);
    box_states.create_box_fragments(line_box, line_info.is_block_in_inline);
    self.context.set_item_index(line_info.end_item_index);

    // Empty items may still leave no line.
    if line_info.is_empty_line {
      return;
    }

    if !line_box_metrics.is_empty() {
      self.container_builder.metrics = line_box_metrics;
    }

    // Blink's `PlaceBlockInInline` sizes the line to the block, which sits at its top.
    if line_info.is_block_in_inline {
      self.container_builder.inline_size = line_info
        .block_in_inline_layout_result()
        .map_or(LayoutUnit::ZERO, |result| result.inline_size);

      return;
    }

    // The items sat on a baseline at zero; the line box's top is zero from here.
    line_box.move_in_block_direction(line_box_metrics.ascent);
    self.container_builder.text_fit_scale = line_info.text_fit_scale;
    self.container_builder.inline_size = inline_size;
  }

  /// Blink's `GetLineClampState`.
  fn line_clamp_state(&self, line_info: &LineInfo<'a, 'c>) -> LineClampState {
    let style = &self.node.block.context.style;

    // Blink's `IsHidden`, which the standard `line-clamp` takumi parses turns on.
    if self.constraint_space.is_past_clamp_point() {
      return LineClampState::Hide;
    }

    if !line_info.is_block_in_inline
      && self.constraint_space.is_at_clamp_point()
      && style.misc_inherited1_data.block_ellipsis != BlockEllipsis::None
    {
      return LineClampState::LineClampEllipsis;
    }

    // Blink's `ShouldTruncateOverflowingText`; takumi's computed `text-overflow` is `clip` in a
    // box that does not clip its overflow.
    let should_truncate_overflowing_text = style.misc4_data.text_overflow != TextOverflow::Clip;

    // Approximate: Blink truncates a line that overflowed before `text-fit` fitted it.
    let has_overflow = !line_info.is_block_in_inline
      && line_info.has_overflow
      && (line_info.text_fit_scale == 1.0 || line_info.width() > line_info.available_width());

    if has_overflow && should_truncate_overflowing_text {
      return LineClampState::TextOverflowEllipsis;
    }

    LineClampState::Show
  }

  /// Blink's `PlaceOutOfFlowObjects`: the static positions of the out-of-flow boxes on the line.
  fn place_out_of_flow_objects(
    &self,
    line_info: &LineInfo<'a, 'c>,
    line_box_metrics: &FontHeight,
    line_box: &mut LogicalLineItems<'a, 'c>,
  ) {
    debug_assert!(line_info.is_empty_line || !line_box_metrics.is_empty());

    let space = self.constraint_space;
    let metrics = if line_info.is_empty_line {
      FontHeight::default()
    } else {
      *line_box_metrics
    };

    // A block-level box sits on the next line, at the container's start edge, past floats or not.
    let block_level_line_location = match space.direction {
      TextDirection::Ltr => LayoutUnit::ZERO,
      TextDirection::Rtl => space.inline_size(),
    };
    let block_level_inline_offset = block_level_line_location
      - (self.container_builder.bfc_line_offset - space.bfc_offset.line_offset);
    let is_ltr = line_info.base_direction == TextDirection::Ltr;
    let mut has_preceding_inline_level_content = false;
    let mut has_rtl_block_level_out_of_flow_objects = false;

    for child in &mut line_box.children {
      has_preceding_inline_level_content |=
        child.has_in_flow_fragment() || child.has_non_empty_inline_box_start;

      let Some(node) = child.out_of_flow_positioned_box else {
        continue;
      };
      let mut static_offset = LogicalOffset::new(LayoutUnit::ZERO, -metrics.ascent);

      if node.is_original_display_inline_type() {
        // An inline-level box sits where it would in flow.
        static_offset.inline_offset = child.rect.offset.inline_offset;
      } else {
        static_offset.inline_offset = block_level_inline_offset;

        // Content before it in logical order moves it down; an RTL line is in visual order, so it
        // counts in the reverse pass below.
        if !is_ltr {
          has_rtl_block_level_out_of_flow_objects = true;
        } else if has_preceding_inline_level_content {
          static_offset.block_offset += metrics.line_height();
        }
      }

      child.rect.offset = static_offset;
    }

    if !has_rtl_block_level_out_of_flow_objects {
      return;
    }

    has_preceding_inline_level_content = false;

    for child in line_box.children.iter_mut().rev() {
      let Some(node) = child.out_of_flow_positioned_box else {
        has_preceding_inline_level_content |=
          child.has_in_flow_fragment() || child.has_non_empty_inline_box_start;

        continue;
      };

      if has_preceding_inline_level_content && !node.is_original_display_inline_type() {
        child.rect.offset.block_offset += metrics.line_height();
      }
    }
  }

  /// Blink's `PlaceFloatingObjects`: places the floats left for the next line, and moves every
  /// float to its place from the line box.
  fn place_floating_objects(
    &mut self,
    line_box_metrics: &FontHeight,
    opportunity: &LineLayoutOpportunity,
    line_info: &LineInfo<'a, 'c>,
    line_box: &mut LogicalLineItems<'a, 'c>,
  ) {
    debug_assert!(line_info.is_empty_line || !line_box_metrics.is_empty());

    let space = self.constraint_space;
    let metrics = if line_info.is_empty_line {
      FontHeight::default()
    } else {
      *line_box_metrics
    };
    // Floats placed after the line start on the next one.
    let origin_bfc_block_offset = opportunity.bfc_block_offset + metrics.line_height();
    let bfc_line_offset = self.container_builder.bfc_line_offset;
    let bfc_block_offset = if line_info.is_empty_line {
      space.bfc_offset.block_offset
    } else {
      line_info.bfc_offset.block_offset
    };

    for child in &mut line_box.children {
      // An empty line placed its floats as leading ones.
      if let Some(node) = child.unpositioned_float.take() {
        let positioned_float = Self::position_float(
          space,
          origin_bfc_block_offset,
          node,
          &mut self.container_builder.exclusion_space,
          &mut *self.atomic_inline,
        );

        child.layout_result = Some(LineFragment::Float {
          node,
          positioned_float,
        });
        child.bfc_offset = positioned_float.bfc_offset;
      }

      if !child.is_floating() {
        continue;
      }

      child.rect.offset = LogicalOffset::new(
        child.bfc_offset.line_offset - bfc_line_offset,
        child.bfc_offset.block_offset - bfc_block_offset - metrics.ascent,
      );
    }
  }

  /// Blink's `AddAnyClearanceAfterLine`: a `<br>` that clears floats adds the space past them
  /// after the line.
  fn add_any_clearance_after_line(&mut self, line_info: &LineInfo<'a, 'c>) {
    let Some(item_result) = line_info.results.last() else {
      return;
    };
    let Some(object) = item_result.item.layout_object else {
      return;
    };

    if !object.is_br() {
      return;
    }

    let line_box_bfc_block_offset = self
      .container_builder
      .line_box_bfc_block_offset
      .unwrap_or(LayoutUnit::ZERO);
    let mut bfc_offset = BfcOffset::new(
      LayoutUnit::ZERO,
      line_box_bfc_block_offset + self.container_builder.line_height(),
    );
    let block_end_offset_without_clearance = bfc_offset.block_offset;
    let clear_type = object
      .style()
      .base_data
      .clear
      .resolve(self.constraint_space.direction);

    if clear_type == Clear::None {
      return;
    }

    adjust_to_clearance(
      self
        .container_builder
        .exclusion_space
        .clearance_offset(clear_type),
      &mut bfc_offset,
    );
    self.container_builder.clearance_after_line =
      bfc_offset.block_offset - block_end_offset_without_clearance;
  }

  /// Blink's `PositionLeadingFloats`: places the floats ahead of the line's content, before it
  /// breaks.
  fn position_leading_floats(
    &mut self,
    exclusion_space: &mut ExclusionSpace,
    leading_floats: &mut LeadingFloats,
  ) {
    let items = &self.items_data.items;
    let mut index = self
      .break_token
      .map_or(0, |break_token| break_token.start.item_index);

    while index < items.len() {
      let item = &items[index];

      if !item.is_empty_item {
        break;
      }

      if item.item_type == InlineItemType::Floating
        && let Some(object) = item.layout_object
      {
        // An empty line's block offset is the expected one.
        leading_floats.floats.push(Self::position_float(
          self.constraint_space,
          self.constraint_space.bfc_offset.block_offset,
          object.node(),
          exclusion_space,
          &mut *self.atomic_inline,
        ));
      }

      index += 1;
    }

    leading_floats.handled_index = index;
  }

  /// Blink's `PositionFloat`.
  fn position_float(
    space: &ConstraintSpace,
    origin_bfc_block_offset: LayoutUnit,
    node: &'c RenderNode,
    exclusion_space: &mut ExclusionSpace,
    atomic_inline: &mut dyn AtomicInlineLayout<'c>,
  ) -> PositionedFloat {
    let mut unpositioned_float = UnpositionedFloat::new(
      node,
      space.inline_size(),
      BfcOffset::new(space.bfc_offset.line_offset, origin_bfc_block_offset),
      space.direction,
    );

    position_float(&mut unpositioned_float, exclusion_space, atomic_inline)
  }
}

impl LineInfo<'_, '_> {
  /// Blink's `ApplyTextAlign`: justifies the line, or returns how far `text-align` moves it.
  fn apply_text_align(&mut self) -> LayoutUnit {
    // `text-indent` can make the width negative; alignment takes it as it is.
    let space = self.available_width() - self.width_for_alignment();
    let mut text_align = self.text_align();

    if text_align == TextAlign::Justify {
      if let Some(offset) = apply_justification(space, self) {
        return offset;
      }

      text_align = TextAlign::Start;
    }

    text_align.line_offset_for_text_align(self.base_direction, space)
  }

  /// Blink's `AdjustLineOffsetForHanging`: an RTL line starts past its hanging spaces.
  fn adjust_line_offset_for_hanging(&self) -> LayoutUnit {
    if self.base_direction == TextDirection::Ltr {
      return LayoutUnit::ZERO;
    }

    -self.hang_width()
  }
}

impl LogicalLineItems<'_, '_> {
  /// Blink's `PlaceRelativePositionedItems`.
  fn place_relative_positioned_items(&mut self, space: &ConstraintSpace) {
    for child in &mut self.children {
      if let Some(node) = child.layout_result.as_ref().map(LineFragment::node) {
        child.rect.offset += space.compute_relative_offset_for_inline(node);
      }
    }
  }
}

impl RenderNode {
  /// Blink's `IsOriginalDisplayInlineType`.
  fn is_original_display_inline_type(&self) -> bool {
    self.context.style.original_display.is_inline_level()
  }
}

impl<'c> InlineNode<'c> {
  /// Blink's `BlockLayoutAlgorithm` laying out its inline child: a line at a time, each below the
  /// last, until the content ends.
  pub(crate) fn layout_lines<'a>(
    self,
    items_data: &'a InlineNodeData<'c>,
    available_inline_size: LayoutUnit,
    atomic_inline: &mut dyn AtomicInlineLayout<'c>,
  ) -> Vec<LineBoxFragment<'a, 'c>> {
    let style = &self.block.context.style;
    let mut space =
      ConstraintSpace::new(Some(available_inline_size), style.inherited_data.direction);

    space.percentage_resolution_block_size = self
      .block
      .context
      .definite_height()
      .map(LayoutUnit::from_float);

    space.lines_until_clamp = style.line_clamp().map(|lines| lines as i32);

    let paragraph_scale = self.measure_paragraph_scale(items_data, space.clone(), atomic_inline);
    let lines = self.layout_lines_in(
      items_data,
      space.clone(),
      Some(paragraph_scale),
      atomic_inline,
    );

    // Blink's `ShouldRelayoutWithNoForcedTruncate`: no line follows the clamp line, so it relays
    // out ignoring the clamp.
    if lines
      .last()
      .is_some_and(|line| line.lines_until_clamp == Some(0))
    {
      space.lines_until_clamp = None;

      return self.layout_lines_in(items_data, space, Some(paragraph_scale), atomic_inline);
    }

    lines
  }

  /// Blink's `LayoutInlineChild`, measuring the scale of a consistent `text-fit` with a layout of
  /// its own.
  fn measure_paragraph_scale(
    self,
    items_data: &InlineNodeData<'c>,
    space: ConstraintSpace,
    atomic_inline: &mut dyn AtomicInlineLayout<'c>,
  ) -> ParagraphScale {
    let text_fit = self.block.context.style.misc_inherited1_data.text_fit;
    let grow_consistent =
      text_fit.mode == TextFitMode::Grow && text_fit.target == TextFitTarget::Consistent;
    let shrink_consistent =
      text_fit.mode == TextFitMode::Shrink && text_fit.target == TextFitTarget::Consistent;

    if !grow_consistent && !shrink_consistent {
      return ParagraphScale::default();
    }

    let lines = self.layout_lines_in(items_data, space.clone(), None, atomic_inline);
    let paragraph_scale = self.measure_per_block_scale(items_data, &lines, space.inline_size());

    if (paragraph_scale.scale < 1.0 && !shrink_consistent)
      || (paragraph_scale.scale > 1.0 && !grow_consistent)
    {
      return ParagraphScale::default();
    }

    paragraph_scale
  }

  fn layout_lines_in<'a>(
    self,
    items_data: &'a InlineNodeData<'c>,
    mut space: ConstraintSpace,
    paragraph_scale: Option<ParagraphScale>,
    atomic_inline: &mut dyn AtomicInlineLayout<'c>,
  ) -> Vec<LineBoxFragment<'a, 'c>> {
    let mut exclusion_space = ExclusionSpace::default();
    let mut context = InlineChildLayoutContext::default();

    context.enable_measuring_mode_if_necessary(paragraph_scale);
    let mut break_token = None;
    let mut lines = Vec::new();

    // Blink's `LayoutWithOptimalInlineChildLayoutContext`.
    if self.block.context.style.inherited_data.text_wrap_style != TextWrapStyle::Auto
      && !items_data.is_score_line_break_disabled
    {
      context.score_line_break_context = Some(ScoreLineBreakContext::default());
    }

    // Blink's `PreviousInflowPosition`: where the last line ended, the margins after it, and
    // whether a line resolved the BFC block offset, before which margins collapse through the
    // block start when they adjoin it.
    let margins_adjoin = self.block.context.margins_adjoin();
    let mut position = LayoutUnit::ZERO;
    let mut margin_strut = MarginStrut::default();
    let mut is_bfc_block_offset_resolved = !margins_adjoin.start;

    loop {
      // A float or an out-of-flow box on an empty line sits below the margins before it.
      let margin_offset = if is_bfc_block_offset_resolved {
        margin_strut.sum()
      } else {
        LayoutUnit::ZERO
      };

      space.bfc_offset.block_offset = position + margin_offset;

      let mut line = InlineLayoutAlgorithm::new(
        self,
        items_data,
        &space,
        &exclusion_space,
        break_token,
        &mut context,
        atomic_inline,
      )
      .layout();

      line.static_bfc_block_offset = space.bfc_offset.block_offset;
      exclusion_space = line.exclusion_space.clone();
      break_token = line.break_token;
      space.lines_until_clamp = line.lines_until_clamp;

      match line
        .block_in_inline_layout_result()
        .and_then(|result| result.collapsed_margins)
      {
        Some(margins) if margins.is_self_collapsing => {
          let start = margin_strut.collapse_with(margins.start);

          if is_bfc_block_offset_resolved {
            line.static_bfc_block_offset = position + start.sum();
          }

          margin_strut = start.collapse_with(margins.end);
        }
        // Blink's `PlaceBlockInInline` keeps the block's `EndMarginStrut`, its start margins
        // collapsing with the ones before it.
        Some(margins) => {
          if is_bfc_block_offset_resolved {
            let start = margin_strut.collapse_with(margins.start).sum();

            line.move_in_block_direction(start - margin_offset);
          }

          is_bfc_block_offset_resolved = true;
          margin_strut = margins.end;
        }
        // A line box resolves the margins before it; an empty line leaves them.
        None if line.line_box_bfc_block_offset.is_some() => {
          is_bfc_block_offset_resolved = true;
          margin_strut = MarginStrut::default();
        }
        None => {}
      }

      if let Some(line_box_bfc_block_offset) = line.line_box_bfc_block_offset {
        position = line_box_bfc_block_offset + line.block_size + line.clearance_after_line;
      }

      line.end_margin_strut = margin_strut;
      lines.push(line);

      if break_token.is_none() {
        return lines;
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use taffy::Line;

  use crate::geometry::{AvailableSpace, Size};
  use crate::layout::{
    inline::{
      atomic_inline::BoxAtomicInline,
      inline_node::InlineNode,
      line_breaker::AtomicInlineLayout,
      testing::{TestAtomicInline, render_tree, span},
    },
    node::Node,
    tree::RenderNode,
  };
  use crate::layout_unit::LayoutUnit;

  /// Each line's line-left offset, top, and inline size, then the text of its items.
  fn layout_lines(block: &RenderNode, available_width: f32) -> Vec<String> {
    layout_lines_with(block, available_width, &mut TestAtomicInline)
  }

  /// [`layout_lines`] with the boxes on the lines laid out by `atomic_inline`.
  fn layout_lines_with<'c>(
    block: &'c RenderNode,
    available_width: f32,
    atomic_inline: &mut dyn AtomicInlineLayout<'c>,
  ) -> Vec<String> {
    let node = InlineNode::new(block);
    let data = node.prepare_layout();

    node
      .layout_lines(
        &data,
        LayoutUnit::from_float(available_width),
        atomic_inline,
      )
      .iter()
      .map(|line| {
        let text: String = line
          .items
          .children
          .iter()
          .filter(|item| item.shape_result.is_some())
          .map(|item| &data.text_content[item.text_offset.clone()])
          .collect();

        format!(
          "{:?} {}@{}+{}",
          text,
          line.bfc_line_offset.to_float(),
          line
            .line_box_bfc_block_offset
            .map_or(-1.0, LayoutUnit::to_float),
          line.inline_size.to_float(),
        )
      })
      .collect()
  }

  /// A span holding an empty block of class `class`.
  fn span_with_block(class: &str) -> Node {
    Node::container([Node::container([]).with_class_name(class)]).with_class_name("span")
  }

  /// The tops of `block`'s lines, the margins at its edges collapsing out where `margins_adjoin`.
  fn line_tops(block: &RenderNode, margins_adjoin: Line<bool>) -> Vec<f32> {
    let mut atomic_inline = BoxAtomicInline {
      available_space: Size {
        width: AvailableSpace::Definite(60.0),
        height: AvailableSpace::MaxContent,
      },
    };

    block.context.set_margins_adjoin(margins_adjoin);

    let node = InlineNode::new(block);
    let data = node.prepare_layout();

    node
      .layout_lines(&data, LayoutUnit::from_float(60.0), &mut atomic_inline)
      .iter()
      .filter_map(|line| line.line_box_bfc_block_offset)
      .map(LayoutUnit::to_float)
      .collect()
  }

  /// WPT's `block-in-inline-after-block-in-inline-with-margin-collapse`: the margins of blocks in
  /// two spans collapse through the empty lines between them.
  #[test]
  fn margins_of_blocks_in_inline_collapse_across_empty_lines() {
    let block = render_tree(
      [span_with_block("first"), span_with_block("second")],
      ".first, .second { display: block; height: 10px } .first { margin-bottom: 50px } \
       .second { margin-top: 20px }",
    );

    assert_eq!(line_tops(&block, Line::FALSE), [0.0, 60.0]);
  }

  /// The margins before the first line box collapse out through the block start they adjoin, and
  /// stay inside when it has a border or padding.
  #[test]
  fn start_margins_collapse_out_where_they_adjoin() {
    let block = render_tree(
      [span_with_block("first"), Node::text("a")],
      ".first { display: block; height: 10px; margin-top: 20px }",
    );

    assert_eq!(line_tops(&block, Line::FALSE), [20.0, 30.0]);
    assert_eq!(
      line_tops(
        &block,
        Line {
          start: true,
          end: false,
        },
      ),
      [0.0, 10.0]
    );
  }

  /// Blink's `PlaceBlockInInline`: the block fills its line from the container's line-left edge,
  /// whatever `text-align` says, and the line takes its height.
  #[test]
  fn a_block_in_inline_fills_its_line() {
    let block = render_tree(
      [Node::container([
        span("ab"),
        Node::container([]).with_class_name("block"),
        span("cd"),
      ])
      .with_class_name("span")],
      ".container { text-align: center } .block { display: block; height: 20px }",
    );
    let mut atomic_inline = BoxAtomicInline {
      available_space: Size {
        width: AvailableSpace::Definite(60.0),
        height: AvailableSpace::MaxContent,
      },
    };

    assert_eq!(
      layout_lines_with(&block, 60.0, &mut atomic_inline),
      ["\"ab\" 24@0+12", "\"\" 0@10+60", "\"cd\" 24@30+12"]
    );
  }

  #[test]
  fn lines_stack_below_each_other() {
    let block = render_tree([Node::text("abc def")], "");

    assert_eq!(
      layout_lines(&block, 30.0),
      ["\"abc\" 0@0+18", "\"def\" 0@10+18"]
    );
  }

  #[test]
  fn text_align_moves_the_line_box() {
    let center = render_tree([Node::text("abc")], ".container { text-align: center }");
    let end = render_tree([Node::text("abc")], ".container { text-align: end }");

    assert_eq!(layout_lines(&center, 60.0), ["\"abc\" 21@0+18"]);
    assert_eq!(layout_lines(&end, 60.0), ["\"abc\" 42@0+18"]);
  }

  /// The two spaces of the first line take its 6px left; the last line aligns to the start.
  #[test]
  fn justify_spreads_the_space_left() {
    let block = render_tree(
      [Node::text("ab cd ef gh")],
      ".container { text-align: justify }",
    );

    assert_eq!(
      layout_lines(&block, 54.0),
      ["\"ab cd ef\" 0@0+54", "\"gh\" 0@10+12"]
    );
  }

  #[test]
  fn text_indent_moves_the_first_line() {
    let block = render_tree([Node::text("abc def")], ".container { text-indent: 12px }");

    assert_eq!(
      layout_lines(&block, 36.0),
      ["\"abc\" 12@0+18", "\"def\" 0@10+18"]
    );
  }

  /// A leading float takes the first lines' start; the line below it starts at the edge.
  #[test]
  fn a_leading_float_narrows_the_lines_beside_it() {
    let block = render_tree(
      [
        Node::container([]).with_class_name("atomic floated"),
        span("abc def ghi"),
      ],
      ".floated { float: left; height: 15px }",
    );

    assert_eq!(
      layout_lines(&block, 42.0),
      ["\"abc\" 18@0+18", "\"def\" 18@10+18", "\"ghi\" 0@20+18"]
    );
  }

  /// A word too wide for the space beside a leading float moves below it, where the line spans
  /// the container.
  #[test]
  fn a_word_too_wide_beside_a_leading_float_moves_below_it() {
    let block = render_tree(
      [
        Node::container([]).with_class_name("atomic floated"),
        span("abcdefg"),
      ],
      ".floated { float: left; height: 15px }",
    );

    assert_eq!(
      layout_lines(&block, 48.0),
      ["\"\" 18@-1+0", "\"abcdefg\" 0@15+42"]
    );
  }

  /// `pretty` moves a word down so the last line is not one short word.
  #[test]
  fn pretty_avoids_a_short_last_word() {
    let greedy = render_tree([Node::text("aaa bbb ccc d")], "");
    let pretty = render_tree(
      [Node::text("aaa bbb ccc d")],
      ".container { text-wrap: pretty }",
    );

    assert_eq!(
      layout_lines(&greedy, 66.0),
      ["\"aaa bbb ccc\" 0@0+66", "\"d\" 0@10+6"]
    );
    assert_eq!(
      layout_lines(&pretty, 66.0),
      ["\"aaa bbb\" 0@0+42", "\"ccc d\" 0@10+30"]
    );
  }

  /// `balance` evens the lines by their scores.
  #[test]
  fn balance_evens_the_lines() {
    let block = render_tree(
      [Node::text("aaa bbb ccc d")],
      ".container { text-wrap: balance }",
    );

    assert_eq!(
      layout_lines(&block, 66.0),
      ["\"aaa bbb\" 0@0+42", "\"ccc d\" 0@10+30"]
    );
  }

  /// Cloned box decorations take scoring away, so `balance` bisects the width instead.
  #[test]
  fn balance_bisects_without_scores() {
    let block = render_tree(
      [span("aaa bbb ccc d").with_class_name("span cloned")],
      ".container { text-wrap: balance } .cloned { box-decoration-break: clone }",
    );

    assert_eq!(
      layout_lines(&block, 66.0),
      ["\"aaa bbb\" 0@0+42", "\"ccc d\" 0@10+30"]
    );
  }
}
