import type { CssInput, Node } from "@takumi-rs/helpers";
import {
  type DocumentInput,
  type FontLoader,
  type ImagesInput,
  type JsonSource,
  lazy,
  needsArgs,
  Prepared,
  type PrepareImagesInput,
  prepareSource,
  readSource,
  renderPrepared,
  resolveDocument,
  sheetsOf,
  toNativeSource,
  WasmRenderer,
} from "@takumi-rs/bindings-js";
import initWasm, {
  type FontCacheOptions,
  type InitInput,
  initSync as initWasmSync,
  Painter as PainterInternal,
  type RegisteredFamily,
  type SyncInitInput,
} from "../pkg/takumi_paint_wasm";
import { type PaintTree, PaintTreeView } from "./tree";

export type { FontLoader, ImagesInput, PrepareImagesInput } from "@takumi-rs/bindings-js";
export { Prepared } from "@takumi-rs/bindings-js";
export type {
  BlendMode,
  ColorStop,
  CornerRadii,
  ElementInfo,
  Filter,
  FontCacheOptions,
  Glyph,
  Gradient,
  ImagePaint,
  ImageSource,
  Matrix,
  Paint,
  Point,
  Rect,
  RegisteredFace,
  RegisteredFamily,
  Rgba,
  Role,
  Sampling,
  Shape,
  Spread,
  Stroke,
} from "../pkg/takumi_paint_wasm";
export type {
  BoxNode,
  Drawable,
  Effects,
  Font,
  ImageNode,
  PaintNode,
  PaintStep,
  PaintTree,
  SerializedPaintTree,
  TextNode,
  TextRun,
} from "./tree";

/** A document input: a takumi node tree, JSX, or an HTML string. */
export type NodeInput = DocumentInput;

/** Options for {@link Painter.paint}. */
export type PaintOptions = {
  /** Canvas width in device pixels. Omit to size the canvas to the content. */
  width?: number;
  /** Canvas height in device pixels. Omit to size the canvas to the content. */
  height?: number;
  /** Device pixels per CSS px; scales every CSS length. @default 1 */
  devicePixelRatio?: number;
  /** Fonts to register before layout, deduped against earlier registrations. */
  fonts?: FontLoader[];
  /** Images keyed by the `src` nodes reference them with. */
  images?: ImagesInput;
  /** CSS to apply before layout: stylesheet text, or rules written as objects. */
  css?: CssInput | readonly CssInput[];
  /** Per-render font stack: ordered family names used as the fallback chain. */
  fontFamilies?: string[];
  /** Default BCP-47 language tag applied to the root. */
  lang?: string;
};

/** Options for {@link Painter.prepare}: everything {@link PaintOptions} lays out with. */
export type PrepareOptions = Omit<PaintOptions, "images"> & {
  /** Images to use as they are, and how to fetch the rest the document draws. */
  images?: PrepareImagesInput;
  signal?: AbortSignal;
};

/** A document {@link Painter.prepare} readied, with its fonts and images loaded. */
export type PreparedSource = Prepared<
  Node | string | JsonSource,
  Omit<PaintOptions, "fonts" | "images" | "fontFamilies" | "css"> & { css?: CssInput[] }
>;

/** What the wasm side reads `input` as: HTML as written, a tree as JSON when it can, or JSX rendered. */
async function resolveNode(
  input: NodeInput,
): Promise<{ node: Node | string | JsonSource; css: string[] }> {
  const { node, css } = await resolveDocument(input);

  return { node: toNativeSource(node), css };
}

/** Loads the wasm module for `takumi-paint/no-init`. The other entries load it on import. */
export default async function init(source: {
  module_or_path: InitInput | Promise<InitInput>;
}): Promise<void> {
  await initWasm(source);
}

/** Loads the wasm module from its bytes or a compiled `WebAssembly.Module`. */
export function initSync(source: { module: SyncInitInput }): void {
  initWasmSync(source);
}

export class Painter extends WasmRenderer<RegisteredFamily, PainterInternal> {
  constructor(options?: FontCacheOptions) {
    super(new PainterInternal(options));
  }

  /** Reads what a node tree, JSX, or HTML needs under `options`, then loads its font subsets and fetches its images. */
  async prepare(node: NodeInput, options: PrepareOptions = {}): Promise<PreparedSource> {
    const { fonts, images, signal, fontFamilies, css, ...rest } = options;
    const main = await resolveNode(node);
    const layout = { ...rest, css: sheetsOf(css, main.css) };

    return prepareSource(
      this.fonts,
      main.node,
      layout,
      { fonts, images, signal, fontFamilies },
      (request) => this.inner.needs(main.node, ...needsArgs(layout, request)),
    );
  }

  /** Lays out a node tree, JSX, an HTML string, or a prepared document and returns what painting it would draw. */
  paint(document: PreparedSource): Promise<PaintTree>;
  paint(node: NodeInput, options?: PaintOptions): Promise<PaintTree>;
  async paint(node: PreparedSource | NodeInput, options: PaintOptions = {}): Promise<PaintTree> {
    const prepared = node instanceof Prepared ? node : await this.read(node, options);
    const painted = await renderPrepared(this.fonts, prepared, {}, (native, opts) =>
      this.inner.paint(native, opts),
    );

    return new PaintTreeView(painted.tree(), painted);
  }

  private async read(node: NodeInput, options: PaintOptions): Promise<PreparedSource> {
    const { fonts, images, css, fontFamilies, ...rest } = options;
    const main = await resolveNode(node);
    const layout = { ...rest, css: sheetsOf(css, main.css) };

    return readSource(this.fonts, main.node, layout, { fonts, images, fontFamilies }, (request) =>
      this.inner.needs(main.node, ...needsArgs(layout, request)),
    );
  }
}

const shared = lazy(() => new Painter());

/** Prepares with a lazily created shared {@link Painter}. */
export function prepare(node: NodeInput, options?: PrepareOptions): Promise<PreparedSource> {
  return shared().prepare(node, options);
}

/** Paints with a lazily created shared {@link Painter}. */
export function paint(document: PreparedSource): Promise<PaintTree>;
export function paint(node: NodeInput, options?: PaintOptions): Promise<PaintTree>;
export function paint(
  node: PreparedSource | NodeInput,
  options?: PaintOptions,
): Promise<PaintTree> {
  return node instanceof Prepared ? shared().paint(node) : shared().paint(node, options);
}
