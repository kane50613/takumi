import type { CssInput } from "@takumi-rs/helpers";
import { fromHtml } from "@takumi-rs/helpers/html";
import { fromJsx } from "@takumi-rs/helpers/jsx";
import type { FontLoader, ImagesInput } from "@takumi-rs/helpers/renderer";
import { FontRegistry } from "@takumi-rs/helpers/renderer";
import {
  LIST_MARKER_CHARACTERS,
  type Node,
  type ReactElementLike,
  subsetFonts,
} from "@takumi-rs/helpers";
import type { ReactNode } from "react";
import initWasm, {
  type InitInput,
  initSync as initWasmSync,
  Painter as PainterInternal,
  type RegisteredFamily,
  type SyncInitInput,
} from "../pkg/takumi_paint_wasm";
import { type PaintTree, PaintTreeView } from "./tree";

export type { FontLoader, ImagesInput } from "@takumi-rs/helpers/renderer";
export type {
  BlendMode,
  ColorStop,
  CornerRadii,
  ElementInfo,
  Filter,
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
  TextNode,
  TextRun,
} from "./tree";

/** A document input: a takumi node tree, JSX, or an HTML string. */
export type NodeInput = Node | ReactNode | ReactElementLike | string;

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

function isNode(value: NodeInput): value is Node {
  return typeof value === "object" && value !== null && "type" in value && !("$$typeof" in value);
}

function isCssList(css: CssInput | readonly CssInput[]): css is readonly CssInput[] {
  return Array.isArray(css);
}

async function resolveNode(input: NodeInput): Promise<{ node: Node; css: string[] }> {
  if (isNode(input)) {
    return { node: input, css: [] };
  }
  if (typeof input === "string") {
    return fromHtml(input);
  }
  return fromJsx(input as ReactNode);
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

export class Painter {
  #inner = new PainterInternal();
  #fonts = new FontRegistry<RegisteredFamily>((font) => this.#inner.registerFont(font));

  /** Lays out a node tree, JSX, or an HTML string and returns what painting it would draw. */
  async paint(node: NodeInput, options: PaintOptions = {}): Promise<PaintTree> {
    const { fonts, images, css, fontFamilies, ...rest } = options;
    const main = await resolveNode(node);
    const resources = await this.#fonts.resolveResources(
      fonts && subsetFonts({ fonts, source: [main.node, LIST_MARKER_CHARACTERS] }),
      images,
      fontFamilies,
    );
    const own = css === undefined ? [] : isCssList(css) ? [...css] : [css];
    const sheets = [...own, ...main.css];

    const painted = this.#inner.paint(main.node, {
      ...rest,
      css: sheets.length > 0 ? sheets : undefined,
      images: resources.images,
      fontFamilies: resources.fontFamilies,
    });

    return new PaintTreeView(painted.tree(), (index) => painted.fontData(index));
  }

  /** Registers a font ahead of time, deduped against earlier registrations. */
  registerFont(font: FontLoader) {
    return this.#fonts.register(font);
  }

  /** Releases the underlying wasm memory. */
  free() {
    this.#inner.free();
  }
}

let shared: Painter | undefined;

/** Paints with a lazily created shared {@link Painter}. */
export function paint(node: NodeInput, options?: PaintOptions): Promise<PaintTree> {
  shared ??= new Painter();
  return shared.paint(node, options);
}
