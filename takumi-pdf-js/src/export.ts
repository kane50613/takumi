import type { CssInput } from "@takumi-rs/helpers";
import {
  collectPdf,
  type DocumentInput,
  type FontLoader,
  type ImagesInput,
  lazy,
  needsArgs,
  type PageSize,
  type PdfPreparedRenderOptions,
  type PdfPrepareOptions,
  type PdfRenderOptions,
  Prepared,
  type PreparedPdf,
  prepareSource,
  readSource,
  renderPrepared,
  resolveNode,
  sheetsOf,
  splitExtras,
  type ViewportInput,
  WasmRenderer,
} from "@takumi-rs/bindings-js";
import {
  PdfRenderer as PdfRendererInternal,
  type RegisteredFamily,
  type RendererOptions,
} from "../pkg/takumi_pdf_wasm";

export { default, initSync } from "../pkg/takumi_pdf_wasm";
export type {
  CacheMode,
  CacheOptions,
  FontCacheOptions,
  RendererOptions,
} from "../pkg/takumi_pdf_wasm";
export type {
  ArchivalAttachment,
  Attachment,
  BandInput,
  Dimensions,
  FontLoader,
  ImagesInput,
  PageMargin,
  PageMarginSide,
  PageOverrideInput,
  PageRange,
  PageRulesInput,
  PageSize,
  PageSizeName,
  PdfMetadata,
  PrepareImagesInput,
  UncoveredText,
  ViewportInput,
  XmpProperty,
  XmpSchema,
} from "@takumi-rs/bindings-js";
export { Prepared } from "@takumi-rs/bindings-js";
export { PageNumber, TargetPageNumber, TotalPages } from "./primitives";
export type { CounterProps, CounterStyle } from "./primitives";

/** A document input: a takumi node tree, JSX, or an HTML string. */
export type NodeInput = DocumentInput;

/** Options for {@link PdfRenderer.render}. */
export type RenderOptions = PdfRenderOptions;

/** Options for {@link PdfRenderer.prepare}: everything {@link RenderOptions} lays out with. */
export type PrepareOptions = PdfPrepareOptions;

/** Options for rendering a {@link PreparedSource}: how its pages are written to the file. */
export type PreparedRenderOptions = PdfPreparedRenderOptions;

/** A document {@link PdfRenderer.prepare} readied, with its fonts and images loaded. */
export type PreparedSource = PreparedPdf;

/**
 * Options for {@link PdfRenderer.measure}: page geometry (or a viewport) plus
 * layout resources. Margins do not affect the result.
 */
export type MeasureOptions = (
  | { size?: PageSize; landscape?: boolean; viewport?: never }
  | { viewport: ViewportInput; size?: never; landscape?: never }
) & {
  /** Fonts to register before layout, deduped across calls. */
  fonts?: FontLoader[];
  /** Pre-fetched images for `src` URLs in the tree. */
  images?: ImagesInput;
  /** CSS to apply before layout, one string or a list cascading in order. */
  css?: CssInput | readonly CssInput[];
  /** Per-render font stack: ordered family names used as the fallback chain. */
  fontFamilies?: string[];
  /** Default BCP-47 language tag applied to the root. */
  lang?: string;
};

/** A node tree's laid-out size in CSS px. */
export type MeasuredSize = { width: number; height: number };

/** A PDF renderer holding registered fonts. Reuse one instance across renders. */
export class PdfRenderer extends WasmRenderer<RegisteredFamily, PdfRendererInternal> {
  constructor(options?: RendererOptions) {
    super(new PdfRendererInternal(options));
  }

  /** Reads what a document and its bands need under `options`, then loads their font subsets and fetches their images. */
  prepare(node: NodeInput, options: PrepareOptions = {}): Promise<PreparedSource> {
    return collectPdf(this.fonts, node, options, prepareSource, (source, layout, request) =>
      this.inner.needs(source, ...needsArgs(layout, request)),
    );
  }

  /** Renders a node tree, JSX, an HTML string, or a prepared document to PDF bytes. */
  render(
    document: PreparedSource,
    options?: PreparedRenderOptions,
  ): Promise<Uint8Array<ArrayBuffer>>;
  render(node: NodeInput, options?: RenderOptions): Promise<Uint8Array<ArrayBuffer>>;
  async render(
    node: PreparedSource | NodeInput,
    options: RenderOptions = {},
  ): Promise<Uint8Array<ArrayBuffer>> {
    return renderPrepared(
      this.fonts,
      node instanceof Prepared ? node : await this.read(node, options),
      node instanceof Prepared ? options : { signal: options.signal },
      (native, opts) => this.inner.render(native, opts),
    );
  }

  /**
   * Lays out a node tree without rendering and returns its size in CSS px.
   *
   * With page options the tree lays out against the full page width with
   * unbounded height, exactly how {@link render} measures a header or footer
   * band (`pageNumber` / `totalPages` hooks are filled with three-digit
   * counters), so the height tells you how much margin a band needs.
   *
   * The returned size is the tree's own, not the space it was laid out
   * against: a box with `width: 100px` measures 100 wide on any page.
   */
  measure(document: PreparedSource): Promise<MeasuredSize>;
  measure(node: NodeInput, options?: MeasureOptions): Promise<MeasuredSize>;
  async measure(
    node: PreparedSource | NodeInput,
    options: MeasureOptions = {},
  ): Promise<MeasuredSize> {
    if (node instanceof Prepared) {
      return renderPrepared(this.fonts, node, {}, (native, opts) =>
        this.inner.measure(native, opts),
      );
    }

    const {
      extras,
      layout: { css, ...rest },
    } = splitExtras(options);
    const main = await resolveNode(node);
    const layout = { ...rest, css: sheetsOf(css, main.css) };

    // The tree measured may itself be a band, so its counters are always in play.
    const prepared = await readSource(this.fonts, main.node, layout, extras, (request) =>
      this.inner.needs(main.node, ...needsArgs(layout, request)),
    );

    return renderPrepared(this.fonts, prepared, {}, (native, opts) =>
      this.inner.measure(native, opts),
    );
  }

  /** A document and its bands read under `options` without fetching, for {@link render}. */
  private read(node: NodeInput, options: RenderOptions): Promise<PreparedSource> {
    return collectPdf(this.fonts, node, options, readSource, (source, layout, request) =>
      this.inner.needs(source, ...needsArgs(layout, request)),
    );
  }
}

const shared = lazy(() => new PdfRenderer());

/** Prepares with a lazily created shared {@link PdfRenderer}. */
export function prepare(node: NodeInput, options?: PrepareOptions): Promise<PreparedSource> {
  return shared().prepare(node, options);
}

/** Renders with a lazily created shared {@link PdfRenderer}. */
export function render(
  document: PreparedSource,
  options?: PreparedRenderOptions,
): Promise<Uint8Array<ArrayBuffer>>;
export function render(node: NodeInput, options?: RenderOptions): Promise<Uint8Array<ArrayBuffer>>;
export function render(
  node: PreparedSource | NodeInput,
  options?: RenderOptions,
): Promise<Uint8Array<ArrayBuffer>> {
  return node instanceof Prepared ? shared().render(node, options) : shared().render(node, options);
}

/** Measures with a lazily created shared {@link PdfRenderer}. */
export function measure(document: PreparedSource): Promise<MeasuredSize>;
export function measure(node: NodeInput, options?: MeasureOptions): Promise<MeasuredSize>;
export function measure(
  node: PreparedSource | NodeInput,
  options?: MeasureOptions,
): Promise<MeasuredSize> {
  return node instanceof Prepared ? shared().measure(node) : shared().measure(node, options);
}
