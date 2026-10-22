//! Blink's [`StyleContentAlignmentData`](https://source.chromium.org/chromium/chromium/src/+/main:third_party/blink/renderer/core/style/style_content_alignment_data.h):
//! the alignment `justify-content` and `align-content` ask of a container's content.

use crate::style::JustifyContent;

/// Blink's `ContentPosition`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContentPosition {
  Normal,
  Start,
  End,
  FlowStart,
  FlowEnd,
  Center,
}

/// Blink's `ContentDistributionType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContentDistribution {
  Default,
  SpaceBetween,
  SpaceAround,
  SpaceEvenly,
  Stretch,
}

/// Blink's `StyleContentAlignmentData`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ContentAlignment {
  pub(crate) position: ContentPosition,
  pub(crate) distribution: ContentDistribution,
  pub(crate) is_safe: bool,
}

impl ContentAlignment {
  pub(crate) fn of(value: JustifyContent) -> Self {
    let (position, distribution, is_safe) = match value {
      JustifyContent::Start => (ContentPosition::Start, ContentDistribution::Default, false),
      JustifyContent::SafeStart => (ContentPosition::Start, ContentDistribution::Default, true),
      JustifyContent::End => (ContentPosition::End, ContentDistribution::Default, false),
      JustifyContent::SafeEnd => (ContentPosition::End, ContentDistribution::Default, true),
      JustifyContent::FlexStart => (
        ContentPosition::FlowStart,
        ContentDistribution::Default,
        false,
      ),
      JustifyContent::SafeFlexStart => (
        ContentPosition::FlowStart,
        ContentDistribution::Default,
        true,
      ),
      JustifyContent::FlexEnd => (
        ContentPosition::FlowEnd,
        ContentDistribution::Default,
        false,
      ),
      JustifyContent::SafeFlexEnd => (ContentPosition::FlowEnd, ContentDistribution::Default, true),
      JustifyContent::Center => (ContentPosition::Center, ContentDistribution::Default, false),
      JustifyContent::SafeCenter => (ContentPosition::Center, ContentDistribution::Default, true),
      JustifyContent::Stretch => (ContentPosition::Normal, ContentDistribution::Stretch, false),
      JustifyContent::SpaceBetween => (
        ContentPosition::Normal,
        ContentDistribution::SpaceBetween,
        false,
      ),
      JustifyContent::SpaceAround => (
        ContentPosition::Normal,
        ContentDistribution::SpaceAround,
        false,
      ),
      JustifyContent::SpaceEvenly => (
        ContentPosition::Normal,
        ContentDistribution::SpaceEvenly,
        false,
      ),
      _ => (ContentPosition::Normal, ContentDistribution::Default, false),
    };

    Self {
      position,
      distribution,
      is_safe,
    }
  }
}
