use std::cell::{RefCell, RefMut};
use std::ops::Range;
use std::rc::Rc;

use crate::krilla::color::rgb;
use crate::krilla::text::Glyph;
use crate::krilla::text::type3::ColoredGlyph;
use crate::krilla::text::{FontContainer, FontIdentifier};

pub(crate) enum GlyphSpan<'a, T>
where
  T: Glyph,
{
  Unspanned(&'a [T]),
  Spanned(&'a [T], &'a str),
}

impl<T> GlyphSpan<'_, T>
where
  T: Glyph,
{
  pub(crate) fn glyphs(&self) -> &[T] {
    match self {
      GlyphSpan::Unspanned(glyphs) => glyphs,
      GlyphSpan::Spanned(glyphs, _) => glyphs,
    }
  }

  pub(crate) fn actual_text(&self) -> Option<&str> {
    match self {
      GlyphSpan::Unspanned(_) => None,
      GlyphSpan::Spanned(_, text) => Some(text),
    }
  }
}

/// In PDF, correspondences between glyphs and Unicode codepoints are expressed
/// via a CMAP. In a CMAP, you can assign a sequence of Unicode codepoints to each
/// glyph. There are two issues with this approach:
/// - How to deal with the fact that the same glyph might be assigned two different codepoints
///   in different contexts (i.e. space and NZWJ).
/// - How to deal with complex shaping scenarios, where there is not a one-to-one or
///   one-to-many correspondence between glyphs and codepoints, but instead a many-to-one
///   or many-to-many mapping.
///
/// The answer to this is the `ActualText` feature of PDF, which allows to define some custom
/// actual text for a number of glyphs, which overrides anything else. Unfortunately, this
/// is seemingly only supported in Acrobat and Chrome, but it's the only proper way of addressing
/// this issue.
///
/// This is the task of the `TextSpanner`. Given a sequence of glyphs, it segments the
/// sequence into subruns of glyphs that either do need to be wrapped in an actual text
/// attribute, or not.
pub(crate) struct GlyphSpanner<'a, T>
where
  T: Glyph,
{
  slice: &'a [T],
  context_color: rgb::Color,
  reversed: bool,
  forbid_invalid_codepoints: bool,
  font_container: Rc<RefCell<FontContainer>>,
  text: &'a str,
}

impl<'a, T> GlyphSpanner<'a, T>
where
  T: Glyph,
{
  pub(crate) fn new(
    slice: &'a [T],
    text: &'a str,
    reversed: bool,
    forbid_invalid_codepoints: bool,
    context_color: rgb::Color,
    font_container: Rc<RefCell<FontContainer>>,
  ) -> Self {
    Self {
      slice,
      context_color,
      reversed,
      forbid_invalid_codepoints,
      text,
      font_container,
    }
  }
}

impl<'a, T> Iterator for GlyphSpanner<'a, T>
where
  T: Glyph,
{
  type Item = GlyphSpan<'a, T>;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    fn func<U>(
      g: &U,
      context_color: rgb::Color,
      previous_range: Option<Range<usize>>,
      forbid_invalid_codepoints: bool,
      mut font_container: RefMut<FontContainer>,
      text: &str,
    ) -> (Range<usize>, bool)
    where
      U: Glyph,
    {
      let (identifier, pdf_glyph) =
        font_container.add_glyph(ColoredGlyph::new(g.glyph_id(), context_color));
      let pdf_font = font_container
        .get_from_identifier_mut(identifier.clone())
        .unwrap();

      let range = g.text_range().clone();
      let text = &text[range.clone()];
      let codepoints = pdf_font.get_codepoints(pdf_glyph);
      // Check if the glyph has already been assigned codepoints that don't match the
      // one we are seeing right now.
      let incompatible_codepoint = codepoints.is_some_and(|c| c != text);

      // Only set the codepoint if there isn't a previous, different mapping.
      //
      // If we could set it, we only want to insert a codepoint if we are not already
      // building a spanned run (which is the case if the previous range is the same).
      // If we are building a spanned run, it means that the glyphs are part of the same
      // cluster, in which case only the first glyph should be assigned the codepoint,
      // while all other glyphs in the same cluster should not be assigned anything.
      // Otherwise, when copying text from the PDF, we will get the same codepoint multiple
      // times in viewers that don't support `ActualText`.
      //
      // However, in case we are for example exporting to PDF/UA, every glyph is required
      // to have a valid codepoint mapping. So in this case, we still add the codepoints
      // to each glyph in the cluster, this will result in worse copy-pasting in viewers
      // that don't support `ActualText`.
      if !incompatible_codepoint
        && (previous_range != Some(range.clone()) || forbid_invalid_codepoints)
        && codepoints != Some(text)
      {
        pdf_font.set_codepoints(pdf_glyph, text.to_string(), g.location());
      }

      (range, incompatible_codepoint)
    }

    let mut use_span = None;
    let mut count = 1;

    let mut iter = self.slice.iter();

    // Get the range of the first glyph, as well as whether it's
    // incompatible.
    let (first_range, first_incompatible) = func(
      iter.next()?,
      self.context_color,
      None,
      self.forbid_invalid_codepoints,
      self.font_container.borrow_mut(),
      self.text,
    );

    let mut prev_range = first_range.clone();

    for next in iter {
      let (next_range, next_incompatible) = func(
        next,
        self.context_color,
        Some(prev_range.clone()),
        self.forbid_invalid_codepoints,
        self.font_container.borrow_mut(),
        self.text,
      );

      match use_span {
        // In this case, we just started and we are looking at the first two glyphs.
        // This decides whether the current run will be spanned, or not.
        None => {
          if prev_range == next_range {
            // The two glyphs are in the same range, so we definitely want this run
            // to be spanned, and also want to include both glyphs in that run.
            use_span = Some(true);
          } else {
            // Else, whether we use a span depends on whether the first glyph
            // is incompatible.
            use_span = Some(first_incompatible);

            // If either the first glyph or the second glyph are incompatible, they
            // need to be in separate runs, since they are not part of the same cluster.
            if first_incompatible || next_incompatible {
              break;
            }

            // If none are incompatible, then `use_span` is false, and we can also
            // include the next glyph in that unspanned run.
          }
        }
        // We are currently building a spanned range, and all glyphs
        // are part of the same cluster.
        Some(true) => {
          // If the next glyph is not part of the same cluster, terminate the current
          // span and don't include the next one.
          if prev_range != next_range {
            break;
          }
        }
        // We are currently building an unspanned range, meaning the
        // glyphs are not part of the same cluster.
        Some(false) => {
          // If the previous and next glyph are part of the same range this means
          // that they are part of the same cluster. This means that the previous
          // AND the next glyph should be part of the upcoming spanned range, not
          // the current one. To exclude the next glyph, we need to do
          // `count -= 1` before terminating.
          if prev_range == next_range {
            count -= 1;
            break;
          }

          // If the next one is incompatible, terminate the
          // current run, since the next one needs to be spanned.
          if next_incompatible {
            break;
          }
        }
      }

      prev_range = next_range;
      count += 1;

      // PDF 1.7 § 14.8.2.3.3 says that `/ReversedChars` show strings shall not contain
      // interior space glyphs. Thus, we check whether we have a glyph that is not the first
      // one and which has a space as its text mapping, and if so, stop after it. That a space
      // is in the same cluster as another codepoint should be exceedingly rare, so we do a
      // literal comparison against `" "` instead of a containment check and prefer to
      // continue otherwise.
      if self.reversed && count > 1 && &self.text[prev_range.clone()] == " " {
        break;
      }
    }

    // If we only had one glyph to begin with (and never entered the for loop), then
    // it should be spanned if its codepoint is incompatible.
    if count == 1 {
      use_span = Some(first_incompatible);
    }

    let (head, tail) = self.slice.split_at(count);
    self.slice = tail;

    let fragment = match use_span.unwrap_or(false) {
      true => GlyphSpan::Spanned(head, &self.text[first_range]),
      false => GlyphSpan::Unspanned(head),
    };

    Some(fragment)
  }
}

pub(crate) struct GlyphGroup<'a, T>
where
  T: Glyph,
{
  pub(crate) font_identifier: FontIdentifier,
  pub(crate) glyphs: &'a [T],
  // This will be stored in normalized form (i.e. at a font size of 1).
  pub(crate) y_offset: f32,
  // This will be stored in normalized form (i.e. at a font size of 1).
  pub(crate) y_advance: f32,
}

impl<'a, T> GlyphGroup<'a, T>
where
  T: Glyph,
{
  pub fn from_props(glyphs: &'a [T], props: GlyphProps) -> Self {
    GlyphGroup {
      font_identifier: props.font_identifier,
      glyphs,
      y_offset: props.y_offset,
      y_advance: props.y_advance,
    }
  }
}

// The GlyphGrouper further segments glyph runs (that already have been segmented
// by `TextSpanner` into subruns that can be encoded as one consecutive run in PDF.
// This is necessary because:
// - The user provides a font for the whole glyph run, but in PDF, the font might
// have to be switched if the glyph maps to a different Type3 font.
// - The glyph contains a y_offset/y_advance, which cannot be expressed as an adjustment
// and requires us to start a new run with a transformation matrix that takes this
// adjustment into account.
pub(crate) struct GlyphGrouper<'a, T>
where
  T: Glyph,
{
  font_container: Rc<RefCell<FontContainer>>,
  context_color: rgb::Color,
  slice: &'a [T],
}

impl<'a, T> GlyphGrouper<'a, T>
where
  T: Glyph,
{
  pub fn new(
    font_container: Rc<RefCell<FontContainer>>,
    context_color: rgb::Color,
    slice: &'a [T],
  ) -> Self {
    Self {
      font_container,
      context_color,
      slice,
    }
  }
}

impl<'a, T> Iterator for GlyphGrouper<'a, T>
where
  T: Glyph,
{
  type Item = GlyphGroup<'a, T>;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    // Guarantees: All glyphs in `head` have the font identifier that is given in
    // `props`, the same size and the same y offset.
    let (head, tail, props) = {
      let mut count = 1;

      let mut iter = self.slice.iter();
      let first = get_glyph_props(
        iter.next()?,
        self.context_color,
        &mut self.font_container.borrow_mut(),
      );

      for next in iter {
        let temp_glyph = get_glyph_props(
          next,
          self.context_color,
          &mut self.font_container.borrow_mut(),
        );

        // If either of those is different, we need to start a new subrun.
        if first.font_identifier != temp_glyph.font_identifier
          || first.y_offset != temp_glyph.y_offset
          || first.y_advance != 0.0
          || temp_glyph.y_advance != 0.0
        {
          break;
        }

        count += 1;
      }

      let (head, tail) = self.slice.split_at(count);
      (head, tail, first)
    };

    self.slice = tail;

    let glyph_group = GlyphGroup::from_props(head, props);

    Some(glyph_group)
  }
}

pub(crate) struct GlyphProps {
  font_identifier: FontIdentifier,
  y_offset: f32,
  y_advance: f32,
}

pub(crate) fn get_glyph_props<U>(
  g: &U,
  context_color: rgb::Color,
  font_container: &mut FontContainer,
) -> GlyphProps
where
  U: Glyph,
{
  // Safe because we've already added all glyphs in the text spanner.
  let font_identifier = font_container
    .font_identifier(ColoredGlyph::new(g.glyph_id(), context_color))
    .unwrap();

  GlyphProps {
    font_identifier,
    y_offset: g.y_offset(1.0),
    y_advance: g.y_advance(1.0),
  }
}

pub fn use_text_spanner(
  glyphs: &[impl Glyph],
  text: &str,
  context_color: rgb::Color,
  font_container: &mut FontContainer,
) -> bool {
  if glyphs.is_empty() {
    return false;
  }

  let mut do_text_span = false;
  // We _might_ need to group glyphs either if they are mapped to different PDF fonts (which
  // can only happen for Type3 fonts, not CID fonts), or if a glyph has a different y/x offset.
  // Note that it's of course possible that we always use the same Type3 font, but we need to
  // be conservative here.

  let mut check_single = |glyph, do_text_span: &mut bool| {
    // As soon as we know that the glyph run requires a text span, we do not insert any codepoints
    // anymore, because otherwise we might unnecessarily pollute the cmap with entries that
    // wouldn't be necessary. The text spanner will then iterate over the all glyphs again
    // and thus take care of inserting cmap entries for any remaining glyphs we ignore now.
    // The only reason we keep going and don't early abort is in order to fully
    // check the `do_glyph_grouping` property.
    if !*do_text_span {
      check_text_span_prop(glyph, text, context_color, font_container, do_text_span);
    }
  };

  for glyphs in glyphs.windows(2) {
    let prev = &glyphs[0];
    let next = &glyphs[1];

    // If two glyphs are part of the same cluster, we definitely need a text span.
    do_text_span |= prev.text_range() == next.text_range();

    check_single(prev, &mut do_text_span);
  }

  // Since windows checks groups of two, we need to manually check the last glyph
  check_single(glyphs.last().unwrap(), &mut do_text_span);

  do_text_span
}

fn check_text_span_prop(
  glyph: &impl Glyph,
  text: &str,
  context_color: rgb::Color,
  font_container: &mut FontContainer,
  do_text_span: &mut bool,
) {
  let (identifier, pdf_glyph) =
    font_container.add_glyph(ColoredGlyph::new(glyph.glyph_id(), context_color));
  let pdf_font = font_container
    .get_from_identifier_mut(identifier.clone())
    .unwrap();

  let range = glyph.text_range().clone();
  let text = &text[range.clone()];
  let codepoints = pdf_font.get_codepoints(pdf_glyph);

  if codepoints.is_some_and(|t| t != text) {
    // If there already exists an entry, then the codepoints are invalid, and we will have to
    // use a text span.
    *do_text_span = true;
  } else if codepoints.is_none() {
    pdf_font.set_codepoints(pdf_glyph, text.to_string(), glyph.location());
  }
}
