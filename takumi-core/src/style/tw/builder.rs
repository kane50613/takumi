use crate::style::*;

#[derive(Debug, Default)]
pub(super) struct TailwindDeclarationBuilder {
  pub(super) declarations: StyleDeclarationBlock,
}

impl TailwindDeclarationBuilder {
  pub(super) fn push(&mut self, declaration: StyleDeclaration, important: bool) {
    self.declarations.push(declaration, important);
  }

  pub(super) fn finish(mut self) -> StyleDeclarationBlock {
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
