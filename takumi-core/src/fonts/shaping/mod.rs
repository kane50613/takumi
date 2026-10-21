//! Blink's `platform/fonts/shaping`.

pub(crate) mod case_mapping_harfbuzz_buffer_filler;
pub(crate) mod glyph_data;
pub(crate) mod glyph_data_range;
pub(crate) mod harfbuzz_shaper;
pub(crate) mod run_segmenter;
pub(crate) mod shape_cache;
pub(crate) mod shape_result;
pub(crate) mod shape_result_run;
pub(crate) mod shape_result_spacing;
pub(crate) mod shape_result_view;
pub(crate) mod shaping_line_breaker;
#[cfg(test)]
pub(crate) mod testing;
