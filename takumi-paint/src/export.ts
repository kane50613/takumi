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
  type Matrix,
  type PaintFont,
  type PaintBackground,
  type PaintBorder,
  type PaintBoxShadows,
  type PaintClip,
  type PaintDecoration,
  type PaintGlyph,
  type PaintImage,
  type PaintInlineBackground,
  type PaintNodeData,
  type PaintOutline,
  type PaintRect,
  type PaintShadow,
  type PaintSource,
  type PaintStroke,
  type PaintTextRunData,
  type PaintTreeData,
  type PaintUnresolvedEffects,
  type Rgba,
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
  PaintGlyph,
  PaintGradientStop,
  PaintImage,
  PaintInlineBackground,
  PaintNodeData,
  PaintOutline,
  PaintRect,
  PaintShadow,
  PaintSource,
  PaintStroke,
  PaintTextRunData,
  PaintTreeData,
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

function fontAt(fonts: readonly PaintFont[], index: number): PaintFont {
  const font = fonts[index];
  if (!font) throw new RangeError(`The paint tree has no font ${index}`);
  return font;
}

/** A shaped text run. Coordinates are relative to the owning node's border box. */
export class PaintTextRun {
  declare readonly text: string;
  /** Start of the run's baseline, x. */
  declare readonly x: number;
  /** The run's baseline, y. */
  declare readonly y: number;
  /** Advance of the run. */
  declare readonly width: number;
  /** Typographic ascent above the baseline. */
  declare readonly ascent: number;
  /** Typographic descent below the baseline. */
  declare readonly descent: number;
  /** Index into {@link PaintTree.fonts}. */
  declare readonly fontIndex: number;
  /** Font size the run was shaped at. */
  declare readonly fontSize: number;
  /**
   * Height of the run's leaded box: the used line height, grown to a fallback face's own height
   * under `line-height: normal`.
   */
  declare readonly lineHeight: number;
  /** `letter-spacing`, already applied to the glyph positions. */
  declare readonly letterSpacing: number;
  declare readonly color: Rgba;
  /** The span's `opacity`. */
  declare readonly opacity: number;
  /** A transform on top of the node's, when text-fit scales the line. */
  declare readonly transform?: Matrix;
  /** Glyphs relative to the run origin. */
  declare readonly glyphs: PaintGlyph[];
  declare readonly decorations?: PaintDecoration[];
  /** `-webkit-text-stroke`, when visible. */
  declare readonly stroke?: PaintStroke;
  /** UTF-8 byte range of the text within the node's inline text. */
  declare readonly textByteRange: [number, number];
  /** The inline span the run came from. */
  declare readonly spanId?: number;
  /** The font instance the run was shaped with. */
  readonly font: PaintFont;

  constructor(run: PaintTextRunData, fonts: readonly PaintFont[]) {
    Object.assign(this, run);
    this.font = fontAt(fonts, run.fontIndex);
  }

  /** Top of the run's ascent: `y - ascent`. */
  get top(): number {
    return this.y - this.ascent;
  }

  /** Bottom of the run's descent: `y + descent`. */
  get bottom(): number {
    return this.y + this.descent;
  }
}

/**
 * One painted box: a compositing group whose `opacity`, `clip`, and `blendMode` apply to
 * everything inside it. Paints in order: `shadows.outer`, `background`, `shadows.inset`, `border`,
 * `image`, `inlineBackgrounds`, `textRuns`, `children`, then `outline`.
 */
export class PaintNode {
  /** The node this box came from; absent for an anonymous box. */
  declare readonly source?: PaintSource;
  /** Border-box width in device pixels. */
  declare readonly width: number;
  /** Border-box height in device pixels. */
  declare readonly height: number;
  /** Left edge of the border box on the canvas. */
  declare readonly x: number;
  /** Top edge of the border box on the canvas. */
  declare readonly y: number;
  /** The content box: the border box inset by border and padding. */
  declare readonly contentBox: PaintRect;
  /**
   * Absolute transform when the box is rotated, scaled, or skewed; absent for a plain
   * translation. Its translation is `x`, `y`.
   */
  declare readonly transform?: Matrix;
  declare readonly opacity: number;
  /** `mix-blend-mode` other than `normal`. */
  declare readonly blendMode?: string;
  /** Whether `isolation: isolate` applies. */
  declare readonly isolate: boolean;
  /** Overflow clip applied to the children. */
  declare readonly clip?: PaintClip;
  /** The background, when it paints a color or a layer. */
  declare readonly background?: PaintBackground;
  /** The border, when any side has width. */
  declare readonly border?: PaintBorder;
  /** `box-shadow` layers, when any. */
  declare readonly shadows?: PaintBoxShadows;
  /** The outline, painted after the children. */
  declare readonly outline?: PaintOutline;
  declare readonly image?: PaintImage;
  /** `text-shadow` layers under every run, later-listed shadows lowest. */
  declare readonly textShadows?: PaintShadow[];
  /** Inline-span backgrounds, one rounded rect per line, outer spans first. */
  declare readonly inlineBackgrounds?: PaintInlineBackground[];
  /** Effects the tree carries as CSS text instead of resolving. */
  declare readonly unresolvedEffects?: PaintUnresolvedEffects;
  /** `text-align` of the inline content, `start` and `end` resolved to `left` or `right`. */
  declare readonly textAlign?: "left" | "right" | "center" | "justify";
  /** Shaped text runs in visual order. */
  readonly textRuns: PaintTextRun[];
  /** Boxes painted after this one, in paint order. */
  readonly children: PaintNode[];

  constructor(node: PaintNodeData, fonts: readonly PaintFont[]) {
    Object.assign(this, node);
    this.textRuns = (node.textRuns ?? []).map((run) => new PaintTextRun(run, fonts));
    this.children = [];
  }

  /** The matrix that places the border box on the canvas: `transform`, or a translation to `x`, `y`. */
  get matrix(): Matrix {
    return this.transform ?? [1, 0, 0, 1, this.x, this.y];
  }
}

/** Everything the backends paint for a document, in paint order. Iterates its nodes. */
export class PaintTree {
  /** Canvas width in device pixels. */
  readonly width: number;
  /** Canvas height in device pixels. */
  readonly height: number;
  /** Font instances the runs were shaped with. */
  readonly fonts: PaintFont[];
  readonly root: PaintNode;

  constructor(tree: PaintTreeData) {
    this.width = tree.width;
    this.height = tree.height;
    this.fonts = tree.fonts;
    this.root = new PaintNode(tree.root, tree.fonts);

    const pending: [PaintNodeData, PaintNode][] = [[tree.root, this.root]];
    for (let entry = pending.pop(); entry; entry = pending.pop()) {
      const [data, node] = entry;
      for (const child of data.children ?? []) {
        const painted = new PaintNode(child, tree.fonts);
        node.children.push(painted);
        pending.push([child, painted]);
      }
    }
  }

  /**
   * Every node in paint order, parents before children. An `outline` paints after the node's
   * children, so an exporter that draws outlines recurses over `children` itself.
   */
  *[Symbol.iterator](): Generator<PaintNode> {
    const pending = [this.root];
    for (let node = pending.pop(); node; node = pending.pop()) {
      yield node;
      for (let index = node.children.length - 1; index >= 0; index--) {
        const child = node.children[index];
        if (child) pending.push(child);
      }
    }
  }

  /** Every text run in paint order, each paired with the node that paints it. */
  *textRuns(): Generator<{ run: PaintTextRun; node: PaintNode }> {
    for (const node of this) {
      for (const run of node.textRuns) yield { run, node };
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
