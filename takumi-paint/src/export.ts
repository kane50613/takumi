import type { CssInput } from "@takumi-rs/helpers";
import { fromHtml } from "@takumi-rs/helpers/html";
import { fromJsx } from "@takumi-rs/helpers/jsx";
import type { FontLoader, ImagesInput, RegisteredFamilyLike } from "@takumi-rs/helpers/renderer";
import { FontRegistry } from "@takumi-rs/helpers/renderer";
import {
  LIST_MARKER_CHARACTERS,
  type Node,
  type ReactElementLike,
  subsetFonts,
} from "@takumi-rs/helpers";
import type { ReactNode } from "react";
import {
  Painter as PainterInternal,
  type PaintNode,
  type PaintTextRun,
  type PaintTree as PaintTreeShape,
} from "../pkg/takumi_paint_wasm";

export { default, initSync } from "../pkg/takumi_paint_wasm";
export type { FontLoader, ImagesInput } from "@takumi-rs/helpers/renderer";
export type {
  Matrix,
  PaintBackground,
  PaintBackgroundLayer,
  PaintBorder,
  PaintBoxShadows,
  PaintClip,
  PaintDecoration,
  PaintFill,
  PaintFont,
  PaintGradientStop,
  PaintImage,
  PaintInlineBackground,
  PaintNode,
  PaintOutline,
  PaintRect,
  PaintTextRun,
  PaintShadow,
  PaintSource,
  PaintUnresolvedEffects,
  Radii,
  Rgba,
} from "../pkg/takumi_paint_wasm";

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

function* nodes(node: PaintNode): Generator<PaintNode> {
  yield node;
  for (const child of node.children ?? []) yield* nodes(child);
}

/** Everything the backends paint for a document, in paint order. Iterates its nodes. */
export class PaintTree implements PaintTreeShape {
  /** Canvas width in device pixels. */
  readonly width: number;
  /** Canvas height in device pixels. */
  readonly height: number;
  readonly root: PaintNode;

  constructor(tree: PaintTreeShape) {
    this.width = tree.width;
    this.height = tree.height;
    this.root = tree.root;
  }

  /** Every node in paint order, starting at the root. */
  [Symbol.iterator](): Generator<PaintNode> {
    return nodes(this.root);
  }

  /** Every text run in paint order, each paired with the node that paints it. */
  *textRuns(): Generator<{ run: PaintTextRun; node: PaintNode }> {
    for (const node of this) {
      for (const run of node.textRuns ?? []) yield { run, node };
    }
  }

  /** The first node whose source `id` matches. */
  find(id: string): PaintNode | undefined {
    for (const node of this) {
      if (node.source?.id === id) return node;
    }
    return undefined;
  }
}

export class Painter {
  private inner = new PainterInternal();
  private fonts = new FontRegistry<RegisteredFamilyLike>(
    (font) => this.inner.registerFont(font) as RegisteredFamilyLike[],
  );

  /** Lays out a node tree, JSX, or an HTML string and returns what painting it would draw. */
  async paint(node: NodeInput, options: PaintOptions = {}): Promise<PaintTree> {
    const { fonts, images, css, fontFamilies, ...rest } = options;
    const main = await resolveNode(node);
    const resources = await this.fonts.resolveResources(
      fonts && subsetFonts({ fonts, source: [main.node, LIST_MARKER_CHARACTERS] }),
      images,
      fontFamilies,
    );
    const own = css === undefined ? [] : isCssList(css) ? [...css] : [css];
    const sheets = [...own, ...main.css];

    return new PaintTree(
      this.inner.paint(main.node, {
        ...rest,
        css: sheets.length > 0 ? sheets : undefined,
        images: resources.images,
        fontFamilies: resources.fontFamilies,
      }),
    );
  }

  /** Registers a font ahead of time, deduped against earlier registrations. */
  registerFont(font: FontLoader) {
    return this.fonts.register(font);
  }

  /** Releases the underlying wasm memory. */
  free() {
    this.inner.free();
  }
}

let shared: Painter | undefined;

/** Paints with a lazily created shared {@link Painter}. */
export function paint(node: NodeInput, options?: PaintOptions): Promise<PaintTree> {
  shared ??= new Painter();
  return shared.paint(node, options);
}
