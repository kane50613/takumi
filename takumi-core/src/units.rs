//! CSS absolute length units, in the 96 dpi pixels layout runs in.
//!
//! Every crate resolves physical units through these, so a `297mm` length and an
//! A4 page reach the same pixel value.

/// One centimetre.
pub const ONE_CM_IN_PX: f32 = 96.0 / 2.54;
/// One millimetre.
pub const ONE_MM_IN_PX: f32 = ONE_CM_IN_PX / 10.0;
/// One quarter-millimetre.
pub const ONE_Q_IN_PX: f32 = ONE_CM_IN_PX / 40.0;
/// One inch.
pub const ONE_IN_PX: f32 = 2.54 * ONE_CM_IN_PX;
/// One point.
pub const ONE_PT_IN_PX: f32 = ONE_IN_PX / 72.0;
/// One pica.
pub const ONE_PC_IN_PX: f32 = ONE_IN_PX / 6.0;
