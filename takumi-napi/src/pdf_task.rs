//! PDF renders of a document and its bands, and the needs pass that reads them first.

use std::sync::Arc;

use napi::bindgen_prelude::*;
use napi_derive::napi;
use takumi_bindings_common::{
  input::{ListedFont, ListedFonts},
  pdf::PdfRenderOptions,
  source_cache::SourceKind,
};
use takumi_core::{Fonts, layout::node::Node};

use crate::{
  NodeSource, deserialize_with_tracing, map_error, parse_html_source, read_options,
  renderer::{RenderNeeds, Renderer, RendererState},
};

/// A document read on the JS thread, its source and bands parsed on a worker.
pub(crate) struct PdfJob {
  input: Option<(NodeSource, PdfRenderOptions)>,
  state: Arc<RendererState>,
  /// The fonts as they were when the call was made, so a later change leaves this one alone.
  fonts: Arc<Fonts>,
}

impl PdfJob {
  /// The document's tree and its options, with HTML bands parsed and `<style>` CSS appended.
  fn read(&mut self) -> Result<(Node, PdfRenderOptions)> {
    let Some((source, mut options)) = self.input.take() else {
      return Err(Error::from_reason("this render already used its input"));
    };
    let (node, document_css) = source.into_parts(&self.state.sources)?;

    options.resolve_bands(document_css, |html| {
      self
        .state
        .bands
        .parse(SourceKind::Html, html, parse_html_source)
    })?;

    Ok((node, options))
  }
}

pub struct PdfRenderTask(PdfJob);

impl Task for PdfRenderTask {
  type Output = Vec<u8>;
  type JsValue = Buffer;

  fn compute(&mut self) -> Result<Self::Output> {
    let (node, options) = self.0.read()?;

    options
      .render(node, &self.0.fonts, &self.0.state.cache)
      .map_err(map_error)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(output.into())
  }
}

pub struct PdfNeedsTask {
  job: PdfJob,
  listed: Option<ListedFonts>,
}

impl Task for PdfNeedsTask {
  type Output = takumi_raster::RenderNeeds;
  type JsValue = RenderNeeds;

  fn compute(&mut self) -> Result<Self::Output> {
    let (node, options) = self.job.read()?;

    options
      .needs(node, Arc::clone(&self.job.fonts), &self.job.state.cache)
      .map_err(map_error)
  }

  fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
    Ok(RenderNeeds::new(output, self.listed.as_ref()))
  }
}

#[napi]
impl Renderer {
  /// Renders a node tree, or HTML, into PDF bytes asynchronously, paged A4 unless `options` say otherwise.
  #[napi(
    ts_args_type = "source: Node | string | { json: string }, options?: PdfBindingOptions, signal?: AbortSignal",
    ts_return_type = "Promise<Buffer<ArrayBuffer>>"
  )]
  pub fn render_pdf(
    &self,
    source: Either<String, Object>,
    options: Option<Object>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<PdfRenderTask>> {
    let options: PdfRenderOptions = read_options(options)?;
    let fonts = self.state.fonts.for_render(&options.font_scope);

    Ok(AsyncTask::with_optional_signal(
      PdfRenderTask(self.pdf_job(source, options, fonts)?),
      signal,
    ))
  }

  /// Reads what a PDF document and its bands need fetched before they render, asynchronously.
  #[napi(
    ts_args_type = "source: Node | string | { json: string }, options?: PdfBindingOptions, fonts?: ListedFont[], signal?: AbortSignal",
    ts_return_type = "Promise<RenderNeeds>"
  )]
  pub fn pdf_needs(
    &self,
    source: Either<String, Object>,
    options: Option<Object>,
    fonts: Option<Object>,
    signal: Option<AbortSignal>,
  ) -> Result<AsyncTask<PdfNeedsTask>> {
    let mut options: PdfRenderOptions = read_options(options)?;
    let listed = fonts
      .map(deserialize_with_tracing::<Vec<ListedFont>>)
      .transpose()?
      .map(|listed| self.state.fonts.read_listed(listed));
    let fonts = self.state.fonts.for_needs(
      &mut options.font_families,
      listed.as_ref(),
      &options.font_scope,
    );

    Ok(AsyncTask::with_optional_signal(
      PdfNeedsTask {
        job: self.pdf_job(source, options, fonts)?,
        listed,
      },
      signal,
    ))
  }
}

impl Renderer {
  fn pdf_job(
    &self,
    source: Either<String, Object>,
    options: PdfRenderOptions,
    fonts: Arc<Fonts>,
  ) -> Result<PdfJob> {
    Ok(PdfJob {
      input: Some((NodeSource::from_js(source)?, options)),
      state: Arc::clone(&self.state),
      fonts,
    })
  }
}
