import type {
  ElementInfo,
  Glyph,
  ImageSource,
  Matrix,
  RawDrawable,
  RawEffects,
  RawFont,
  RawPaintTree,
  RawPaintNode,
  RawTextRun,
  Rect,
  Shape,
} from "../pkg/takumi_paint_wasm";

/** Something to draw, in the owning node's local space. */
export type Drawable = RawDrawable<TextRun>;

/**
 * How a node composites. Render the enclosed steps into a layer, run `filters` over it, clip it
 * to `clip`, multiply it by the alpha of `mask`, then blend it with `opacity` and `blendMode`.
 */
export type Effects = RawEffects<Drawable>;

/** A font face at one set of variation coordinates. Runs shaped with the same face share one object. */
export interface Font extends RawFont {
  /** The font file, for drawing `glyphs` with a native text API. */
  data(): Uint8Array;
}

/** Text shaped with one font and size. Its fields describe the text for exporters that lay it out again. */
export interface TextRun {
  /** The final text, after `text-transform` and any inserted ellipsis. */
  readonly text: string;
  /** The inline element the run came from, such as a `<b>`; absent for the block's own text. */
  readonly element?: ElementInfo;
  /** Start of the baseline, in the text node's local space. */
  readonly x: number;
  readonly y: number;
  readonly width: number;
  /** Index of the line the run sits on, from `0`. */
  readonly line: number;
  readonly ascent: number;
  readonly descent: number;
  readonly font: Font;
  readonly fontSize: number;
  readonly lineHeight: number;
  /** Already applied to the glyph positions. */
  readonly letterSpacing: number;
  /** Glyph ids in `font`, positioned from the baseline start. */
  readonly glyphs: readonly Glyph[];
  /**
   * The outlines of the glyphs the run's color fills, as SVG path data positioned from the
   * baseline start. Color font layers arrive as their own drawables, and bitmap glyphs such as
   * some emoji are left out.
   */
  outline(): string;
  /** Present when `text-fit` scales the line; maps the run's space before it is placed at `x`, `y`. */
  readonly transform?: Matrix;
}

export type PaintNode = BoxNode | TextNode | ImageNode;

/** Every node is a rectangle placed on the canvas. Its local space runs from (0, 0) to (width, height). */
interface NodeBase {
  /** The element the node belongs to. A text node belongs to the element whose text it lays out. */
  readonly element: ElementInfo | undefined;
  /** Maps local space onto the canvas. */
  readonly transform: Matrix;
  readonly width: number;
  readonly height: number;
  /** The node's rectangle on the canvas, after `transform`. */
  readonly bounds: Rect;
  readonly parent: BoxNode | undefined;
  readonly drawables: readonly Drawable[];
}

/** A CSS box. Iterating it yields the box and its descendants in document order. */
export interface BoxNode extends NodeBase, Iterable<PaintNode> {
  readonly type: "box";
  /** The box inset by border and padding, in local space. */
  readonly contentBox: Rect;
  /** Drawn after the descendants. */
  readonly outline: readonly Drawable[];
  /** Present when the box composites as a group. */
  readonly effects: Effects | undefined;
  /** Clips the descendants, from `overflow`. */
  readonly overflowClip: Shape | undefined;
  /** Boxes, text, and images the element contains, in document order. */
  readonly children: readonly PaintNode[];
}

/** One paragraph of laid-out text. Its local space is the border box of the block its lines fill. */
export interface TextNode extends NodeBase {
  readonly type: "text";
  /** `start` and `end` resolved against the direction. */
  readonly textAlign: "left" | "right" | "center" | "justify";
  /** In visual order. */
  readonly runs: readonly TextRun[];
}

/** A replaced image. Its local space is the content box. */
export interface ImageNode extends NodeBase {
  readonly type: "image";
  readonly image: ImageSource;
}

/** One step of painting the tree, bottom to top. */
export type PaintStep =
  /** Draw these in order, under `node.transform`. */
  | { readonly type: "draw"; readonly node: PaintNode; readonly drawables: readonly Drawable[] }
  /** Start a layer for `node`; composite it per `effects` at the matching `end-group`. */
  | { readonly type: "begin-group"; readonly node: BoxNode; readonly effects: Effects }
  | { readonly type: "end-group"; readonly node: BoxNode }
  /** Clip the enclosed steps to `clip`, in `node`'s local space. */
  | { readonly type: "begin-clip"; readonly node: BoxNode; readonly clip: Shape }
  | { readonly type: "end-clip"; readonly node: BoxNode };

type RawNode<Type extends RawPaintNode["type"]> = Extract<RawPaintNode, { type: Type }>;

/** A painted tree. Lengths are device pixels. Iterating it yields every node in document order. */
export class PaintTree implements Iterable<PaintNode> {
  readonly width: number;
  readonly height: number;
  /** Every font the runs use. */
  readonly fonts: readonly Font[];
  readonly #raw: RawPaintTree;
  readonly #nodes: readonly PaintNode[];

  /** Wraps the serialized tree `raw`, reading font files through `fontData`. */
  constructor(raw: RawPaintTree, fontData: (index: number) => Uint8Array | undefined) {
    this.#raw = raw;
    this.width = raw.width;
    this.height = raw.height;
    this.fonts = raw.fonts.map((font, index) => ({
      ...font,
      data: () => fontData(index) ?? new Uint8Array(),
    }));
    this.#nodes = raw.nodes.map((node) => {
      switch (node.type) {
        case "box":
          return new BoxView(node, this);
        case "text":
          return new TextView(node, this);
        case "image":
          return new ImageView(node, this);
      }
    });
  }

  /** The node listed at `index`. */
  node(index: number): PaintNode {
    const node = this.#nodes[index];

    if (!node) throw new Error(`The tree lists no node ${index}`);
    return node;
  }

  /** The first node, the box every other node descends from. */
  get root(): BoxNode {
    const root = this.node(0);

    if (root.type !== "box") throw new Error("The tree's root is not a box");
    return root;
  }

  /** The box of the element with this `id`. */
  find(id: string): BoxNode | undefined {
    for (const node of this) {
      if (node.type === "box" && node.element?.id === id) return node;
    }
    return undefined;
  }

  /** The steps a renderer takes, in paint order. */
  *paintSteps(): Generator<PaintStep> {
    for (const step of this.#raw.steps) {
      const node = this.node(step.node);

      if (step.type === "draw") {
        yield {
          type: "draw",
          node,
          drawables: step.part === "outline" && node.type === "box" ? node.outline : node.drawables,
        };
      } else if (node.type === "box") {
        if (step.type === "begin-group") {
          if (node.effects) yield { type: step.type, node, effects: node.effects };
        } else if (step.type === "begin-clip") {
          if (node.overflowClip) yield { type: step.type, node, clip: node.overflowClip };
        } else {
          yield { type: step.type, node };
        }
      }
    }
  }

  [Symbol.iterator](): Iterator<PaintNode> {
    return this.root[Symbol.iterator]();
  }
}

/** A node read from its raw form, its links resolved through the tree. */
abstract class NodeView<Raw extends RawPaintNode> {
  protected readonly raw: Raw;
  protected readonly tree: PaintTree;
  #drawables: readonly Drawable[] | undefined;

  constructor(raw: Raw, tree: PaintTree) {
    this.raw = raw;
    this.tree = tree;
  }

  get element(): ElementInfo | undefined {
    return this.raw.element;
  }

  get transform(): Matrix {
    return this.raw.transform;
  }

  get width(): number {
    return this.raw.width;
  }

  get height(): number {
    return this.raw.height;
  }

  get bounds(): Rect {
    return this.raw.bounds;
  }

  get parent(): BoxNode | undefined {
    const parent = this.raw.parent === undefined ? undefined : this.tree.node(this.raw.parent);

    return parent?.type === "box" ? parent : undefined;
  }

  get drawables(): readonly Drawable[] {
    this.#drawables ??= this.raw.drawables.map((drawable) => this.resolve(drawable));
    return this.#drawables;
  }

  /** `drawable` with its glyph run named by the run itself. */
  protected resolve(drawable: RawDrawable): Drawable {
    if (drawable.type !== "glyphs") return drawable;

    throw new Error("Only a text node draws glyphs");
  }
}

class BoxView extends NodeView<RawNode<"box">> implements BoxNode {
  readonly type = "box";
  #outline: readonly Drawable[] | undefined;
  #effects: Effects | undefined;

  get contentBox(): Rect {
    return this.raw.contentBox;
  }

  get outline(): readonly Drawable[] {
    this.#outline ??= this.raw.outline.map((drawable) => this.resolve(drawable));
    return this.#outline;
  }

  get effects(): Effects | undefined {
    const { effects } = this.raw;

    if (!effects) return undefined;
    this.#effects ??= {
      ...effects,
      mask: effects.mask?.map((drawable) => this.resolve(drawable)),
    };
    return this.#effects;
  }

  get overflowClip(): Shape | undefined {
    return this.raw.overflowClip;
  }

  get children(): readonly PaintNode[] {
    return this.raw.children.map((child) => this.tree.node(child));
  }

  *[Symbol.iterator](): Iterator<PaintNode> {
    yield this;
    for (const child of this.children) {
      if (child.type === "box") yield* child;
      else yield child;
    }
  }
}

class TextView extends NodeView<RawNode<"text">> implements TextNode {
  readonly type = "text";
  #runs: readonly TextRun[] | undefined;

  get textAlign(): TextNode["textAlign"] {
    return this.raw.textAlign;
  }

  get runs(): readonly TextRun[] {
    this.#runs ??= this.raw.runs.map((run) => this.wrap(run));
    return this.#runs;
  }

  protected override resolve(drawable: RawDrawable): Drawable {
    if (drawable.type !== "glyphs") return drawable;

    const run = this.runs[drawable.run];

    if (!run) throw new Error(`The text node has no run ${drawable.run}`);
    return { ...drawable, run };
  }

  private wrap(raw: RawTextRun): TextRun {
    const { outline, font, ...fields } = raw;
    const resolved = this.tree.fonts[font];

    if (!resolved) throw new Error(`The tree lists no font ${font}`);
    return { ...fields, font: resolved, outline: () => outline };
  }
}

class ImageView extends NodeView<RawNode<"image">> implements ImageNode {
  readonly type = "image";

  get image(): ImageSource {
    return this.raw.image;
  }
}
