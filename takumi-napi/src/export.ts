import type {
  CacheStats,
  MeasuredNode,
  Node,
  RegisteredFamily,
  RenderAnimationOptions as RenderAnimationOptionsInternal,
  RenderOptions as RenderOptionsInternal,
  RendererOptions,
  SvgRenderOptions as SvgRenderOptionsInternal,
} from "../index";
export type * from "../index";
import { Renderer as RendererInternal } from "../index";

export { setGlyphCacheMaxBytes } from "../index";

import {
  collectPdf,
  FontRegistry,
  Prepared,
  prepareSource,
  needsArgs,
  readSource,
  splitExtras,
  renderPrepared,
  toNativeSource,
} from "@takumi-rs/bindings-js";
import type {
  BackendAnimationOptions,
  BackendPrepareAnimationOptions,
  BackendPreparedRenderOptions,
  BackendPrepareOptions,
  BackendRenderOptions,
  BackendSvgOptions,
  DocumentInput,
  FontLoader,
  JsonSource,
  PdfBindingOptions,
  PdfPreparedRenderOptions,
  PdfPrepareOptions,
  PdfRenderOptions,
  PreparedPdf,
  RegisteredFamilyLike,
} from "@takumi-rs/bindings-js";

export type {
  AnimationOutputFormatOptions,
  FontLoader,
  ImageLoader,
  ImagesInput,
  FallbackFontsLoader,
  OutputFormatOptions,
  PrepareImagesInput,
  ArchivalAttachment,
  Attachment,
  BandInput,
  Dimensions,
  DocumentInput,
  PageMargin,
  PageMarginSide,
  PageOverrideInput,
  PageRange,
  PageRulesInput,
  PageSize,
  PageSizeName,
  PdfMetadata,
  PdfPreparedRenderOptions,
  PdfPrepareOptions,
  PdfRenderOptions,
  PreparedPdf,
  UncoveredText,
  ViewportInput,
  XmpProperty,
  XmpSchema,
} from "@takumi-rs/bindings-js";
export { Prepared } from "@takumi-rs/bindings-js";

export type RenderOptions = BackendRenderOptions<RenderOptionsInternal>;
export type RenderAnimationOptions = BackendAnimationOptions<RenderAnimationOptionsInternal>;
export type SvgRenderOptions = BackendSvgOptions<SvgRenderOptionsInternal>;
export type PrepareOptions = BackendPrepareOptions<RenderOptionsInternal>;
export type PreparedRenderOptions = BackendPreparedRenderOptions<RenderOptionsInternal>;
export type PrepareAnimationOptions =
  BackendPrepareAnimationOptions<RenderAnimationOptionsInternal>;

/** A source {@link Renderer.prepare} readied, with its fonts and images loaded. */
export type PreparedSource = Prepared<
  Node | string | JsonSource,
  Omit<RenderOptionsInternal, "images" | "fontFamilies">
>;

/** An animation {@link Renderer.prepareAnimation} readied, with its fonts and images loaded. */
export type PreparedAnimation = Prepared<
  RenderAnimationOptionsInternal["scenes"],
  Omit<RenderAnimationOptionsInternal, "images" | "fontFamilies" | "scenes">
>;

export class Renderer {
  private inner: RendererInternal;
  private fonts: FontRegistry<RegisteredFamily>;

  constructor(options?: RendererOptions) {
    this.inner = new RendererInternal(options);
    this.fonts = new FontRegistry(this.inner);
  }

  /** Reads what `source` needs under `options`, then loads its font subsets and fetches its images. */
  prepare(source: Node | string, options?: PrepareOptions): Promise<PreparedSource> {
    const { extras, layout } = splitExtras(options ?? {});
    const native = toNativeSource(source);

    return prepareSource(this.fonts, native, layout, extras, (request) =>
      this.inner.needs(native, ...needsArgs(layout, request), taskSignal(extras.signal)),
    );
  }

  render(source: PreparedSource, options?: PreparedRenderOptions): Promise<Buffer<ArrayBuffer>>;
  render(source: Node | string, options?: RenderOptions): Promise<Buffer<ArrayBuffer>>;
  async render(
    source: PreparedSource | Node | string,
    options: RenderOptions = {},
  ): Promise<Buffer<ArrayBuffer>> {
    return renderPrepared(
      this.fonts,
      source instanceof Prepared ? source : await this.read(source, options),
      source instanceof Prepared ? options : { signal: options.signal },
      (native, opts, signal) => this.inner.render(native, opts, taskSignal(signal)),
    );
  }

  renderSvg(source: PreparedSource, options?: { signal?: AbortSignal }): Promise<string>;
  renderSvg(source: Node | string, options?: SvgRenderOptions): Promise<string>;
  async renderSvg(
    source: PreparedSource | Node | string,
    options: SvgRenderOptions = {},
  ): Promise<string> {
    return renderPrepared(
      this.fonts,
      source instanceof Prepared ? source : await this.read(source, options),
      source instanceof Prepared ? options : { signal: options.signal },
      (native, opts, signal) => this.inner.renderSvg(native, opts, taskSignal(signal)),
    );
  }

  measure(source: PreparedSource, options?: { signal?: AbortSignal }): Promise<MeasuredNode>;
  measure(source: Node | string, options?: RenderOptions): Promise<MeasuredNode>;
  async measure(
    source: PreparedSource | Node | string,
    options: RenderOptions = {},
  ): Promise<MeasuredNode> {
    return renderPrepared(
      this.fonts,
      source instanceof Prepared ? source : await this.read(source, options),
      source instanceof Prepared ? options : { signal: options.signal },
      (native, opts, signal) => this.inner.measure(native, opts, taskSignal(signal)),
    );
  }

  /** Reads what a PDF document and its bands need under `options`, then loads their font subsets and fetches their images. */
  preparePdf(source: DocumentInput, options: PdfPrepareOptions = {}): Promise<PreparedPdf> {
    return this.readPdf(source, options, prepareSource);
  }

  /** Renders a node tree, JSX, an HTML string, or a prepared document to PDF bytes. */
  renderPdf(source: PreparedPdf, options?: PdfPreparedRenderOptions): Promise<Buffer<ArrayBuffer>>;
  renderPdf(source: DocumentInput, options?: PdfRenderOptions): Promise<Buffer<ArrayBuffer>>;
  async renderPdf(
    source: PreparedPdf | DocumentInput,
    options: PdfRenderOptions = {},
  ): Promise<Buffer<ArrayBuffer>> {
    return renderPrepared(
      this.fonts,
      source instanceof Prepared ? source : await this.readPdf(source, options, readSource),
      source instanceof Prepared ? options : { signal: options.signal },
      (native, opts, signal) =>
        this.inner.renderPdf(native, opts satisfies PdfBindingOptions, taskSignal(signal)),
    );
  }

  /** A PDF document and its bands read under `options` with `collect`, which fetches their images when it prepares them. */
  private readPdf(
    source: DocumentInput,
    options: PdfPrepareOptions | PdfRenderOptions,
    collect: typeof prepareSource,
  ): Promise<PreparedPdf> {
    return collectPdf(this.fonts, source, options, collect, (native, layout, request, signal) => {
      const [needsOptions, fonts] = needsArgs(layout, request);

      return this.inner.pdfNeeds(
        native,
        needsOptions satisfies PdfBindingOptions,
        fonts,
        taskSignal(signal),
      );
    });
  }

  /** `source` read under `options` without fetching, for {@link render} and its siblings. */
  private read(
    source: Node | string,
    options: RenderOptions | SvgRenderOptions,
  ): Promise<PreparedSource> {
    const { extras, layout } = splitExtras(options);
    const native = toNativeSource(source);

    return readSource(this.fonts, native, layout, extras, (request) =>
      this.inner.needs(native, ...needsArgs(layout, request), taskSignal(extras.signal)),
    );
  }

  /** Reads what every scene needs under `options`, then loads their font subsets and fetches their images. */
  prepareAnimation(options: PrepareAnimationOptions): Promise<PreparedAnimation> {
    return this.readAnimation(options, prepareSource);
  }

  renderAnimation(
    source: PreparedAnimation,
    options?: { signal?: AbortSignal },
  ): Promise<Buffer<ArrayBuffer>>;
  renderAnimation(options: RenderAnimationOptions): Promise<Buffer<ArrayBuffer>>;
  async renderAnimation(
    source: PreparedAnimation | RenderAnimationOptions,
    options: { signal?: AbortSignal } = {},
  ): Promise<Buffer<ArrayBuffer>> {
    const prepared =
      source instanceof Prepared ? source : await this.readAnimation(source, readSource);

    return renderPrepared(
      this.fonts,
      prepared,
      { signal: source instanceof Prepared ? options.signal : source.signal },
      (scenes, opts, signal) => this.inner.renderAnimation({ ...opts, scenes }, taskSignal(signal)),
    );
  }

  /** The scenes of `options` read with `collect`, which fetches their images when it prepares them. */
  private readAnimation(
    options: PrepareAnimationOptions | RenderAnimationOptions,
    collect: typeof prepareSource,
  ): Promise<PreparedAnimation> {
    const {
      extras,
      layout: { scenes, ...layout },
    } = splitExtras(options);
    const sources = scenes.map((scene) => ({ ...scene, node: toNativeSource(scene.node) }));

    return collect(this.fonts, sources, layout, extras, (request) =>
      this.inner.animationNeeds(
        ...needsArgs({ ...layout, scenes: sources }, request),
        taskSignal(extras.signal),
      ),
    );
  }

  registerFont(font: FontLoader): Promise<RegisteredFamily[]> {
    return this.fonts.register(font);
  }

  /**
   * Removes every face registered under the family `name`, from any `registerFont` call and each
   * `unicodeRange` subset alike; returns whether it had any. Takes a family `registerFont` returned
   * as well. A later render that lists one of its fonts registers it again.
   */
  unregisterFont(family: RegisteredFamilyLike): boolean {
    return this.inner.unregisterFont(family.name);
  }

  /** What the renderer's caches hold and how their lookups went since it was created. */
  cacheStats(): CacheStats {
    return this.inner.cacheStats();
  }

  /** Removes every registered font, leaving the bundled fallback font. */
  clearFonts(): void {
    this.inner.clearFonts();
  }
}

/** A fresh signal following `signal` for one native task, since napi-rs replaces its `onabort` and stops cancelling at the first finished task sharing it. */
function taskSignal(signal: AbortSignal | undefined): AbortSignal | undefined {
  if (!signal) {
    return undefined;
  }

  // https://nodejs.org/api/globals.html#static-method-abortsignalanysignals arrived in Node 18.17.
  if (typeof AbortSignal.any === "function") {
    return AbortSignal.any([signal]);
  }

  const controller = new AbortController();

  if (signal.aborted) {
    controller.abort(signal.reason);
  } else {
    signal.addEventListener("abort", () => controller.abort(signal.reason), { once: true });
  }

  return controller.signal;
}
