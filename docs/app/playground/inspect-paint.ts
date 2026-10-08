/**
 * Flattens a takumi-paint tree into plain data the page can hold: one entry per
 * node with its frame on the canvas and its properties already worded, so the
 * inspector draws frames and lists layers without the tree's live objects.
 */
import type {
  Drawable,
  Effects,
  Filter,
  Gradient,
  Matrix,
  Paint,
  PaintNode,
  PaintTree,
  Rect,
  Rgba,
  Shape,
} from "takumi-paint";

type Point = { x: number; y: number };

/** A rectangle's corners after a transform, clockwise from its top-left. */
export type Quad = [Point, Point, Point, Point];

type PropertyRow = {
  label: string;
  value: string;
  /** A CSS `background` previewing the row's paint. */
  swatch?: string;
};

type PropertySection = { title: string; rows: PropertyRow[] };

export type InspectedNode = {
  kind: PaintNode["type"];
  /** Stays the same across renders of the same markup, so a selection survives Run. */
  key: string;
  name: string;
  /** The element's classes, shown beside its name. */
  detail?: string;
  parent?: number;
  depth: number;
  hasChildren: boolean;
  quad: Quad;
  bounds: Rect;
  /** The box inside border and padding, when it differs from the frame. */
  contentQuad?: Quad;
  /** One quad per text run; a paragraph is hit where its text is, not across its block. */
  runQuads?: Quad[];
  /** The step that last draws the node; absent for a node that draws nothing itself. */
  paintOrder?: number;
  sections: PropertySection[];
};

export type PaintInspection = {
  width: number;
  height: number;
  nodes: InspectedNode[];
  /** How many nodes past {@link MAX_NODES} were left out. */
  omitted: number;
};

const MAX_NODES = 4000;

const NAME_LENGTH = 40;

function apply([a, b, c, d, e, f]: Matrix, { x, y }: Point): Point {
  return { x: a * x + c * y + e, y: b * x + d * y + f };
}

function quadOf(matrix: Matrix, { x, y, width, height }: Rect): Quad {
  return [
    apply(matrix, { x, y }),
    apply(matrix, { x: x + width, y }),
    apply(matrix, { x: x + width, y: y + height }),
    apply(matrix, { x, y: y + height }),
  ];
}

function multiply([a1, b1, c1, d1, e1, f1]: Matrix, [a2, b2, c2, d2, e2, f2]: Matrix): Matrix {
  return [
    a1 * a2 + c1 * b2,
    b1 * a2 + d1 * b2,
    a1 * c2 + c1 * d2,
    b1 * c2 + d1 * d2,
    a1 * e2 + c1 * f2 + e1,
    b1 * e2 + d1 * f2 + f1,
  ];
}

/** Whether `point` lies inside the convex `quad`, edges included. */
function quadContains(quad: Quad, point: Point): boolean {
  let sign = 0;

  for (let index = 0; index < 4; index++) {
    const from = quad[index];
    const to = quad[(index + 1) % 4];

    if (!from || !to) return false;

    const cross = (to.x - from.x) * (point.y - from.y) - (to.y - from.y) * (point.x - from.x);

    if (cross === 0) continue;
    if (sign === 0) sign = Math.sign(cross);
    else if (Math.sign(cross) !== sign) return false;
  }

  return true;
}

/** The indices above node `index`, nearest first. */
export function ancestorsOf(nodes: InspectedNode[], index: number): number[] {
  const ancestors: number[] = [];

  for (let parent = nodes[index]?.parent; parent !== undefined; parent = nodes[parent]?.parent) {
    ancestors.push(parent);
  }

  return ancestors;
}

/**
 * The node a click at `point` means: the last one painted there, or the deepest frame
 * inside it that draws nothing itself, such as a padded box without a background.
 */
export function nodeAt({ nodes }: PaintInspection, point: Point): number | undefined {
  const hits = nodes.flatMap((node, index) => {
    const hit = node.runQuads
      ? node.runQuads.some((quad) => quadContains(quad, point))
      : quadContains(node.quad, point);

    return hit ? [index] : [];
  });
  const painted = hits.reduce<number | undefined>((best, index) => {
    const order = nodes[index]?.paintOrder;
    const bestOrder = best === undefined ? undefined : nodes[best]?.paintOrder;

    return order !== undefined && (bestOrder === undefined || order >= bestOrder) ? index : best;
  }, undefined);
  const framed = hits.filter(
    (index) =>
      nodes[index]?.paintOrder === undefined &&
      (painted === undefined || ancestorsOf(nodes, index).includes(painted)),
  );

  return framed.at(-1) ?? painted;
}

function truncate(text: string, length: number) {
  const flat = text.replace(/\s+/g, " ").trim();

  return flat.length > length ? `${flat.slice(0, length - 1)}…` : flat;
}

function round(value: number) {
  return Math.round(value * 100) / 100;
}

function hex(value: number) {
  return Math.round(value).toString(16).padStart(2, "0");
}

function cssColor([r, g, b, a]: Rgba) {
  return `rgba(${r}, ${g}, ${b}, ${round(a / 255)})`;
}

function colorText([r, g, b, a]: Rgba) {
  const code = `#${hex(r)}${hex(g)}${hex(b)}`.toUpperCase();

  return a === 255 ? code : `${code} ${Math.round((a / 255) * 100)}%`;
}

function describeGradient(gradient: Gradient) {
  const stops = gradient.stops
    .map((stop) => `${cssColor(stop.color)} ${round(stop.offset * 100)}%`)
    .join(", ");
  const direction = gradient.type === "linear-gradient" ? "90deg, " : "";

  return {
    value: `${gradient.type.replace("-gradient", "")} · ${gradient.stops.length} stops${gradient.spread === "repeat" ? " · repeat" : ""}`,
    swatch: `${gradient.type}(${direction}${stops})`,
  };
}

function sourceName(src: string) {
  if (src.startsWith("data:")) return `data: ${src.slice(5, src.search(/[;,]/))}`;
  if (src.includes("<svg")) return "inline svg";

  return truncate(src.slice(src.lastIndexOf("/") + 1) || src, NAME_LENGTH);
}

function describePaint(paint: Paint): Omit<PropertyRow, "label"> {
  switch (paint.type) {
    case "color":
      return { value: colorText(paint.color), swatch: cssColor(paint.color) };
    case "linear-gradient":
    case "radial-gradient":
    case "conic-gradient":
      return describeGradient(paint);
    case "image":
      return { value: `image · ${sourceName(paint.image.src)}` };
    case "pattern": {
      const tile = describePaint(paint.tile);
      const count = paint.x.length * paint.y.length;

      return count === 1
        ? tile
        : {
            value: `${count} tiles of ${round(paint.tileWidth)} × ${round(paint.tileHeight)} · ${tile.value}`,
            swatch: tile.swatch,
          };
    }
  }
}

function describeShape(shape: Shape) {
  switch (shape.type) {
    case "rect":
      return "rect";
    case "rounded-rect": {
      const radii = [
        shape.radii.topLeft,
        shape.radii.topRight,
        shape.radii.bottomRight,
        shape.radii.bottomLeft,
      ].map((radius) => round(radius.x));

      return new Set(radii).size === 1 ? `rounded ${radii[0]}` : `rounded ${radii.join(" ")}`;
    }
    case "path":
      return "path";
  }
}

function describeDrawable(drawable: Drawable): PropertyRow {
  switch (drawable.type) {
    case "fill": {
      const { value, swatch } = describePaint(drawable.paint);

      return {
        label: drawable.role,
        value: `${value} · ${describeShape(drawable.shape)}${drawable.blendMode ? ` · ${drawable.blendMode}` : ""}`,
        swatch,
      };
    }
    case "stroke": {
      const { value, swatch } = describePaint(drawable.paint);

      return {
        label: drawable.role,
        value: `${round(drawable.stroke.width)}px ${drawable.stroke.dash ? "dashed" : "solid"} · ${value}`,
        swatch,
      };
    }
    case "shadow":
      return {
        label: drawable.role,
        value: `${colorText(drawable.color)} · ${round(drawable.offset.x)} ${round(drawable.offset.y)} blur ${round(drawable.blur)}${drawable.visible === "inside" ? " · inset" : ""}`,
        swatch: cssColor(drawable.color),
      };
    case "glyphs": {
      const { value, swatch } = describePaint(drawable.paint);

      return {
        label: drawable.role,
        value: `"${truncate(drawable.run.text, 24)}" · ${value}${drawable.blur > 0 ? ` · blur ${round(drawable.blur)}` : ""}`,
        swatch,
      };
    }
    case "image":
      return {
        label: drawable.role,
        value: `${sourceName(drawable.image.src)} · ${drawable.image.width} × ${drawable.image.height}${drawable.sampling === "pixelated" ? " · pixelated" : ""}`,
      };
    case "masked":
      return {
        label: drawable.role,
        value: `${drawable.content.length} drawn through ${drawable.mask.length} mask`,
      };
    case "group":
      return {
        label: "group",
        value: `${drawable.drawables.length} at ${round(drawable.opacity * 100)}%`,
      };
  }
}

function describeFilter(filter: Filter) {
  switch (filter.type) {
    case "blur":
      return `blur ${round(filter.radius)}`;
    case "color-matrix":
      return "color matrix";
    case "drop-shadow":
      return `drop-shadow ${round(filter.offset.x)} ${round(filter.offset.y)} blur ${round(filter.blur)} ${colorText(filter.color)}`;
    case "unsupported":
      return `unsupported: ${filter.css}`;
  }
}

function effectRows(effects: Effects): PropertyRow[] {
  const rows: PropertyRow[] = [];

  if (effects.opacity < 1)
    rows.push({ label: "opacity", value: `${round(effects.opacity * 100)}%` });
  if (effects.blendMode !== "normal") rows.push({ label: "blend", value: effects.blendMode });
  if (effects.isolation) rows.push({ label: "isolation", value: "isolate" });
  for (const filter of effects.filters)
    rows.push({ label: "filter", value: describeFilter(filter) });
  for (const filter of effects.backdropFilters)
    rows.push({ label: "backdrop", value: describeFilter(filter) });
  if (effects.clip) rows.push({ label: "clip", value: describeShape(effects.clip) });
  if (effects.mask) rows.push({ label: "mask", value: `${effects.mask.length} drawn` });

  return rows;
}

function nodeName(node: PaintNode) {
  if (node.type === "text") {
    return `"${truncate(node.runs.map((run) => run.text).join(""), NAME_LENGTH)}"`;
  }

  if (node.type === "image") return sourceName(node.image.src);

  const tag = node.element?.tagName ?? "box";

  return node.element?.id ? `${tag}#${node.element.id}` : tag;
}

function layoutRows(node: PaintNode): PropertyRow[] {
  const [a, b] = node.transform;
  const rotation = round((Math.atan2(b, a) * 180) / Math.PI);
  const rows: PropertyRow[] = [
    { label: "x", value: `${round(node.bounds.x)}` },
    { label: "y", value: `${round(node.bounds.y)}` },
    { label: "size", value: `${round(node.width)} × ${round(node.height)}` },
  ];

  if (rotation !== 0) rows.push({ label: "rotation", value: `${rotation}°` });

  return rows;
}

function sections(node: PaintNode): PropertySection[] {
  const result: PropertySection[] = [{ title: "Layout", rows: layoutRows(node) }];

  if (node.element) {
    const { tagName, id, className, path } = node.element;
    const rows: PropertyRow[] = [];

    if (tagName) rows.push({ label: "tag", value: tagName });
    if (id) rows.push({ label: "id", value: id });
    if (className) rows.push({ label: "class", value: className });
    rows.push({ label: "path", value: path.length > 0 ? path.join(" › ") : "root" });
    result.push({ title: "Element", rows });
  }

  if (node.type === "text") {
    result.push({
      title: "Text",
      rows: [
        { label: "align", value: node.textAlign },
        ...node.runs.map((run) => ({
          label: `line ${run.line + 1}`,
          value: `"${truncate(run.text, NAME_LENGTH)}" · ${run.font.family ?? "unnamed"} ${run.font.weight} · ${round(run.fontSize)}px`,
        })),
      ],
    });
  }

  if (node.type === "image") {
    result.push({
      title: "Image",
      rows: [
        { label: "src", value: sourceName(node.image.src) },
        { label: "natural", value: `${node.image.width} × ${node.image.height}` },
      ],
    });
  }

  const drawn = node.drawables.map(describeDrawable);

  if (node.type === "box") drawn.push(...node.outline.map(describeDrawable));
  if (drawn.length > 0) result.push({ title: "Draws", rows: drawn });

  if (node.type === "box") {
    const rows = node.effects ? effectRows(node.effects) : [];

    if (node.overflowClip)
      rows.push({ label: "overflow", value: `clips to ${describeShape(node.overflowClip)}` });
    if (rows.length > 0) result.push({ title: "Effects", rows });
  }

  return result;
}

function runQuads(node: PaintNode): Quad[] | undefined {
  if (node.type !== "text") return undefined;

  return node.runs.map((run) => {
    const placed: Matrix = [1, 0, 0, 1, run.x, run.y];
    const local = run.transform ? multiply(placed, run.transform) : placed;

    return quadOf(multiply(node.transform, local), {
      x: 0,
      y: -run.ascent,
      width: run.width,
      height: run.ascent + run.descent,
    });
  });
}

function contentQuad(node: PaintNode): Quad | undefined {
  if (node.type !== "box") return undefined;

  const { contentBox, width, height } = node;
  const inset =
    contentBox.x !== 0 ||
    contentBox.y !== 0 ||
    contentBox.width !== width ||
    contentBox.height !== height;

  return inset ? quadOf(node.transform, contentBox) : undefined;
}

export function inspectPaint(tree: PaintTree): PaintInspection {
  const kept = tree.nodes.slice(0, MAX_NODES);
  const indices = new Map<PaintNode, number>(kept.map((node, index) => [node, index]));
  const paintOrders = new Map<PaintNode, number>();
  const nodes: InspectedNode[] = [];

  tree.steps.forEach((step, order) => {
    if (step.type === "draw" && step.drawables.length > 0) paintOrders.set(step.node, order);
  });

  // A parent is listed before its children, so its depth is already known.
  kept.forEach((node, index) => {
    const parent = node.parent ? indices.get(node.parent) : undefined;
    const depth = parent === undefined ? 0 : (nodes[parent]?.depth ?? 0) + 1;

    nodes.push({
      kind: node.type,
      key: node.element ? `${node.type}:${node.element.path.join(".")}` : `${node.type}#${index}`,
      name: nodeName(node),
      detail: node.type === "box" ? node.element?.className : undefined,
      parent,
      depth,
      hasChildren: node.type === "box" && node.children.length > 0,
      quad: quadOf(node.transform, { x: 0, y: 0, width: node.width, height: node.height }),
      bounds: node.bounds,
      contentQuad: contentQuad(node),
      runQuads: runQuads(node),
      paintOrder: paintOrders.get(node),
      sections: sections(node),
    });
  });

  return {
    width: tree.width,
    height: tree.height,
    nodes,
    omitted: tree.nodes.length - kept.length,
  };
}
