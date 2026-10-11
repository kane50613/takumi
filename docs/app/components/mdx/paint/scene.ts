/**
 * A paint tree flattened into plain data a client component can hold: nodes by index, steps
 * that name them, and glyph runs with their outlines already read.
 */
import type {
  BlendMode,
  Drawable,
  Matrix,
  PaintNode,
  PaintTree,
  Rect,
  Shape,
  TextRun,
} from "takumi-paint";
import { inspectPaint, type Quad } from "~/playground/inspect-paint";

type SceneRun = {
  text: string;
  tagName?: string;
  x: number;
  y: number;
  width: number;
  line: number;
  ascent: number;
  descent: number;
  fontSize: number;
  lineHeight: number;
  family?: string;
  weight: number;
  transform?: Matrix;
  outline: string;
};

type Swap<Union, Type, Next> = Union extends { type: Type } ? Next : Union;

export type SceneDrawable = Swap<
  Swap<
    Swap<
      Drawable,
      "glyphs",
      Omit<Extract<Drawable, { type: "glyphs" }>, "run"> & { run: SceneRun }
    >,
    "masked",
    Omit<Extract<Drawable, { type: "masked" }>, "mask" | "content"> & {
      mask: SceneDrawable[];
      content: SceneDrawable[];
    }
  >,
  "group",
  Omit<Extract<Drawable, { type: "group" }>, "drawables"> & { drawables: SceneDrawable[] }
>;

type SceneNode = {
  kind: PaintNode["type"];
  name: string;
  depth: number;
  transform: Matrix;
  width: number;
  height: number;
  bounds: Rect;
  quad: Quad;
  drawables: SceneDrawable[];
  runs: SceneRun[];
};

export type SceneStep =
  | { type: "draw"; node: number; drawables: SceneDrawable[] }
  | { type: "begin-group"; node: number; opacity: number; blendMode: BlendMode }
  | { type: "end-group"; node: number }
  | { type: "begin-clip"; node: number; clip: Shape }
  | { type: "end-clip"; node: number };

export type PaintScene = {
  width: number;
  height: number;
  nodes: SceneNode[];
  steps: SceneStep[];
};

function sceneRun(run: TextRun): SceneRun {
  return {
    text: run.text,
    tagName: run.element?.tagName,
    x: run.x,
    y: run.y,
    width: run.width,
    line: run.line,
    ascent: run.ascent,
    descent: run.descent,
    fontSize: run.fontSize,
    lineHeight: run.lineHeight,
    family: run.font.family,
    weight: run.font.weight,
    transform: run.transform,
    outline: run.outline(),
  };
}

function sceneDrawable(drawable: Drawable): SceneDrawable {
  switch (drawable.type) {
    case "glyphs":
      return { ...drawable, run: sceneRun(drawable.run) };
    case "masked":
      return {
        ...drawable,
        mask: drawable.mask.map(sceneDrawable),
        content: drawable.content.map(sceneDrawable),
      };
    case "group":
      return { ...drawable, drawables: drawable.drawables.map(sceneDrawable) };
    default:
      return drawable;
  }
}

export function paintScene(tree: PaintTree): PaintScene {
  const inspected = inspectPaint(tree).nodes;
  const indices = new Map(tree.nodes.map((node, index) => [node, index]));
  const indexOf = (node: PaintNode) => indices.get(node) ?? 0;

  return {
    width: tree.width,
    height: tree.height,
    nodes: tree.nodes.map((node, index) => ({
      kind: node.type,
      name: inspected[index]?.name ?? node.type,
      depth: inspected[index]?.depth ?? 0,
      transform: node.transform,
      width: node.width,
      height: node.height,
      bounds: node.bounds,
      quad: inspected[index]?.quad ?? [
        { x: 0, y: 0 },
        { x: 0, y: 0 },
        { x: 0, y: 0 },
        { x: 0, y: 0 },
      ],
      drawables: node.drawables.map(sceneDrawable),
      runs: node.type === "text" ? node.runs.map(sceneRun) : [],
    })),
    steps: tree.steps.flatMap((step): SceneStep[] => {
      const node = indexOf(step.node);

      switch (step.type) {
        case "draw":
          return step.drawables.length > 0
            ? [{ type: "draw", node, drawables: step.drawables.map(sceneDrawable) }]
            : [];
        case "begin-group":
          return [
            {
              type: "begin-group",
              node,
              opacity: step.effects.opacity,
              blendMode: step.effects.blendMode,
            },
          ];
        case "begin-clip":
          return [{ type: "begin-clip", node, clip: step.clip }];
        case "end-group":
        case "end-clip":
          return [{ type: step.type, node }];
      }
    }),
  };
}
