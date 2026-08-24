use crate::style::*;

#[derive(Debug, Default)]
pub(super) struct TailwindDeclarationBuilder {
  pub(super) declarations: StyleDeclarationBlock,
  pub(super) transform_state: TwTransformState,
  pub(super) shadow: Option<Vec<BoxShadow>>,
  pub(super) shadow_important: bool,
  pub(super) shadow_color: Option<ColorInput>,
  pub(super) text_shadow: Option<Vec<TextShadow>>,
  pub(super) text_shadow_important: bool,
  pub(super) text_shadow_color: Option<ColorInput>,
  pub(super) filter: Option<Filters>,
  pub(super) filter_important: bool,
  pub(super) backdrop_filter: Option<Filters>,
  pub(super) backdrop_filter_important: bool,
  pub(super) grid_column: TwGridLineState,
  pub(super) grid_row: TwGridLineState,
}

#[derive(Debug, Default)]
pub(super) struct TwGridLineState {
  pub(super) start: Option<GridPlacement>,
  pub(super) end: Option<GridPlacement>,
  pub(super) start_important: bool,
  pub(super) end_important: bool,
}

impl TwGridLineState {
  fn set_line(&mut self, grid_line: GridLine, start_important: bool, end_important: bool) {
    self.set_start(grid_line.start, start_important);
    self.set_end(grid_line.end, end_important);
  }

  fn set_start(&mut self, grid_placement: GridPlacement, important: bool) {
    self.start = Some(grid_placement);
    self.start_important = important;
  }

  fn set_end(&mut self, grid_placement: GridPlacement, important: bool) {
    self.end = Some(grid_placement);
    self.end_important = important;
  }

  fn push_declarations(
    self,
    declarations: &mut StyleDeclarationBlock,
    start_decl: fn(GridPlacement) -> StyleDeclaration,
    end_decl: fn(GridPlacement) -> StyleDeclaration,
  ) {
    if let Some(start) = self.start {
      declarations.push(start_decl(start), self.start_important);
    }

    if let Some(end) = self.end {
      declarations.push(end_decl(end), self.end_important);
    }
  }
}

impl TailwindDeclarationBuilder {
  pub(super) fn push(&mut self, declaration: StyleDeclaration, important: bool) {
    self.declarations.push(declaration, important);
  }

  pub(super) fn set_shadow_layers<I: IntoIterator<Item = BoxShadow>>(
    &mut self,
    layers: I,
    important: bool,
  ) {
    let mut shadows: Vec<BoxShadow> = layers.into_iter().collect();
    if let Some(color) = self.shadow_color {
      for shadow in &mut shadows {
        shadow.color = color;
      }
    }

    self.shadow = Some(shadows);
    self.shadow_important = important;
  }

  pub(super) fn reset_shadow(&mut self, important: bool) {
    self.shadow = None;
    self.shadow_color = None;
    self.shadow_important = important;
  }

  pub(super) fn reset_text_shadow(&mut self, important: bool) {
    self.text_shadow = None;
    self.text_shadow_color = None;
    self.text_shadow_important = important;
  }

  pub(super) fn set_shadow_color(&mut self, color: ColorInput, important: bool) {
    self.shadow_color = Some(color);
    self.shadow_important = important;

    if let Some(shadows) = self.shadow.as_mut() {
      for shadow in shadows {
        shadow.color = color;
      }
    }
  }

  pub(super) fn set_text_shadow_layers<I: IntoIterator<Item = TextShadow>>(
    &mut self,
    layers: I,
    important: bool,
  ) {
    let mut shadows: Vec<TextShadow> = layers.into_iter().collect();
    if let Some(color) = self.text_shadow_color {
      for shadow in &mut shadows {
        shadow.color = color;
      }
    }

    self.text_shadow = Some(shadows);
    self.text_shadow_important = important;
  }

  pub(super) fn set_text_shadow_color(&mut self, color: ColorInput, important: bool) {
    self.text_shadow_color = Some(color);
    self.text_shadow_important = important;

    if let Some(shadows) = self.text_shadow.as_mut() {
      for shadow in shadows {
        shadow.color = color;
      }
    }
  }

  pub(super) fn push_filter(&mut self, filter: Filter, important: bool) {
    self
      .filter
      .get_or_insert_with(Filters::default)
      .push(filter);
    self.filter_important = important;
  }

  pub(super) fn push_backdrop_filter(&mut self, filter: Filter, important: bool) {
    self
      .backdrop_filter
      .get_or_insert_with(Filters::default)
      .push(filter);
    self.backdrop_filter_important = important;
  }

  pub(super) fn set_filter_reset(&mut self, important: bool, use_backdrop: bool) {
    let (filter, filter_important) = if use_backdrop {
      (
        &mut self.backdrop_filter,
        &mut self.backdrop_filter_important,
      )
    } else {
      (&mut self.filter, &mut self.filter_important)
    };
    *filter = Some(Filters::default());
    *filter_important = important;
  }

  pub(super) fn set_grid_column(&mut self, grid_line: GridLine, important: bool) {
    self.grid_column.set_line(grid_line, important, important);
  }

  pub(super) fn set_grid_row(&mut self, grid_line: GridLine, important: bool) {
    self.grid_row.set_line(grid_line, important, important);
  }

  pub(super) fn finish(mut self) -> StyleDeclarationBlock {
    if let Some(shadows) = self.shadow.take() {
      self.push(
        StyleDeclaration::box_shadow(Some(shadows.into_boxed_slice())),
        self.shadow_important,
      );
    }

    if let Some(shadows) = self.text_shadow.take() {
      self.push(
        StyleDeclaration::text_shadow(Some(shadows.into_boxed_slice())),
        self.text_shadow_important,
      );
    }

    if let Some(filter) = self.filter.take() {
      self.push(StyleDeclaration::filter(filter), self.filter_important);
    }

    if let Some(backdrop_filter) = self.backdrop_filter.take() {
      self.push(
        StyleDeclaration::backdrop_filter(backdrop_filter),
        self.backdrop_filter_important,
      );
    }

    self.transform_state.apply(&mut self.declarations);

    self.grid_column.push_declarations(
      &mut self.declarations,
      StyleDeclaration::grid_column_start,
      StyleDeclaration::grid_column_end,
    );
    self.grid_row.push_declarations(
      &mut self.declarations,
      StyleDeclaration::grid_row_start,
      StyleDeclaration::grid_row_end,
    );

    type BorderSide = (LonghandId, LonghandId, fn(BorderStyle) -> StyleDeclaration);
    let sides: [BorderSide; 4] = [
      (
        LonghandId::BorderTopWidth,
        LonghandId::BorderTopStyle,
        StyleDeclaration::border_top_style,
      ),
      (
        LonghandId::BorderRightWidth,
        LonghandId::BorderRightStyle,
        StyleDeclaration::border_right_style,
      ),
      (
        LonghandId::BorderBottomWidth,
        LonghandId::BorderBottomStyle,
        StyleDeclaration::border_bottom_style,
      ),
      (
        LonghandId::BorderLeftWidth,
        LonghandId::BorderLeftStyle,
        StyleDeclaration::border_left_style,
      ),
    ];
    for (width_id, style_id, style_decl) in sides {
      let has_width = self
        .declarations
        .iter()
        .any(|d| d.affected_longhands().contains(&width_id));
      let has_style = self
        .declarations
        .iter()
        .any(|d| d.affected_longhands().contains(&style_id));
      if has_width && !has_style {
        self
          .declarations
          .push(style_decl(BorderStyle::Solid), false);
      }
    }

    self.declarations
  }
}

#[derive(Debug, Default)]
pub(super) struct TwTransformState {
  translate: Option<SpacePair<Length>>,
  translate_important: bool,
  scale: Option<SpacePair<PercentageNumber>>,
  scale_important: bool,
}

impl TwTransformState {
  pub(super) fn set_translate(&mut self, value: SpacePair<Length>, important: bool) {
    self.translate = Some(value);
    self.translate_important = important;
  }

  pub(super) fn translate_mut(&mut self, important: bool) -> &mut SpacePair<Length> {
    self.translate_important = important;
    self
      .translate
      .get_or_insert_with(SpacePair::<Length>::default)
  }

  pub(super) fn set_scale(&mut self, value: SpacePair<PercentageNumber>, important: bool) {
    self.scale = Some(value);
    self.scale_important = important;
  }

  pub(super) fn scale_mut(&mut self, important: bool) -> &mut SpacePair<PercentageNumber> {
    self.scale_important = important;
    self
      .scale
      .get_or_insert_with(SpacePair::<PercentageNumber>::default)
  }

  fn apply(self, declarations: &mut StyleDeclarationBlock) {
    if let Some(translate) = self.translate {
      declarations.push(
        StyleDeclaration::translate(translate),
        self.translate_important,
      );
    }

    if let Some(scale) = self.scale {
      declarations.push(StyleDeclaration::scale(scale), self.scale_important);
    }
  }
}
