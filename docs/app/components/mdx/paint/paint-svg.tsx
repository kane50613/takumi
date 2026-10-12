/**
 * Draws a paint scene as SVG, one element per drawable. Approximate: conic gradients fall back to
 * their first stop, and an image paint outside a pattern draws nothing.
 */
import type { CSSProperties, ReactNode } from "react";
import type { CornerRadii, Gradient, Matrix, Paint, Rect, Shape, Stroke } from "takumi-paint";
import { cssColor } from "~/playground/inspect-paint";
import type { PaintScene, SceneDrawable } from "./scene";

/** Hands out ids for the gradients, filters, clips, and masks one SVG defines. */
export class SvgIds {
  #prefix: string;
  #next = 0;

  constructor(prefix: string) {
    this.#prefix = prefix.replace(/[^a-zA-Z0-9_-]/g, "");
  }

  next() {
    return `${this.#prefix}-${this.#next++}`;
  }
}

export function matrixCss([a, b, c, d, e, f]: Matrix) {
  return `matrix(${a}, ${b}, ${c}, ${d}, ${e}, ${f})`;
}

function roundedRectPath({ x, y, width, height }: Rect, radii: CornerRadii) {
  const { topLeft: tl, topRight: tr, bottomRight: br, bottomLeft: bl } = radii;

  return [
    `M${x + tl.x},${y}`,
    `H${x + width - tr.x}`,
    `A${tr.x},${tr.y} 0 0 1 ${x + width},${y + tr.y}`,
    `V${y + height - br.y}`,
    `A${br.x},${br.y} 0 0 1 ${x + width - br.x},${y + height}`,
    `H${x + bl.x}`,
    `A${bl.x},${bl.y} 0 0 1 ${x},${y + height - bl.y}`,
    `V${y + tl.y}`,
    `A${tl.x},${tl.y} 0 0 1 ${x + tl.x},${y}`,
    "Z",
  ].join(" ");
}

function shapeElement(shape: Shape, props: Record<string, unknown> = {}): ReactNode {
  switch (shape.type) {
    case "rect":
      return <rect {...shape.rect} {...props} />;
    case "rounded-rect":
      return <path d={roundedRectPath(shape.rect, shape.radii)} {...props} />;
    case "path":
      return <path d={shape.d} fillRule={shape.fillRule} {...props} />;
  }
}

function gradientElement(id: string, gradient: Gradient): ReactNode {
  const stops = gradient.stops.map((stop, index) => (
    <stop key={index} offset={stop.offset} stopColor={cssColor(stop.color)} />
  ));
  const spreadMethod = gradient.spread === "repeat" ? "repeat" : "pad";

  if (gradient.type === "linear-gradient") {
    return (
      <linearGradient
        id={id}
        gradientUnits="userSpaceOnUse"
        spreadMethod={spreadMethod}
        x1={gradient.start.x}
        y1={gradient.start.y}
        x2={gradient.end.x}
        y2={gradient.end.y}
      >
        {stops}
      </linearGradient>
    );
  }

  if (gradient.type === "radial-gradient") {
    const { center, radiusX, radiusY } = gradient;

    return (
      <radialGradient
        id={id}
        gradientUnits="userSpaceOnUse"
        spreadMethod={spreadMethod}
        cx={0}
        cy={0}
        r={1}
        fr={gradient.start}
        gradientTransform={`translate(${center.x} ${center.y}) scale(${radiusX} ${radiusY})`}
      >
        {stops}
      </radialGradient>
    );
  }

  return undefined;
}

/** `paint` as an SVG paint value, with any definition it needs. */
function paintValue(paint: Paint, ids: SvgIds): { value: string; defs?: ReactNode } {
  switch (paint.type) {
    case "color":
      return { value: cssColor(paint.color) };
    case "linear-gradient":
    case "radial-gradient": {
      const id = ids.next();

      return { value: `url(#${id})`, defs: gradientElement(id, paint) };
    }
    case "conic-gradient":
      return { value: paint.stops[0] ? cssColor(paint.stops[0].color) : "none" };
    case "image":
    case "pattern":
      return { value: "none" };
  }
}

/** One tile of `pattern` at every `x` and `y`. */
function patternTiles(pattern: Extract<Paint, { type: "pattern" }>, ids: SvgIds): ReactNode {
  const { tile, tileWidth: width, tileHeight: height } = pattern;

  return pattern.y.flatMap((y) =>
    pattern.x.map((x) => {
      if (tile.type === "image") {
        return (
          <image
            key={`${x},${y}`}
            href={tile.image.src}
            x={x}
            y={y}
            width={width}
            height={height}
            preserveAspectRatio="none"
          />
        );
      }

      const { value, defs } = paintValue(tile, ids);

      return (
        <g key={`${x},${y}`} transform={`translate(${x} ${y})`}>
          {defs && <defs>{defs}</defs>}
          <rect width={width} height={height} fill={value} />
        </g>
      );
    }),
  );
}

function strokeProps(stroke: Stroke) {
  return {
    strokeWidth: stroke.width,
    strokeDasharray: stroke.dash?.join(" "),
    strokeLinecap: stroke.cap,
    strokeLinejoin: stroke.join,
  };
}

function clipped(shapes: readonly Shape[] | undefined, ids: SvgIds, children: ReactNode) {
  return (shapes ?? []).reduceRight<ReactNode>((inner, shape) => {
    const id = ids.next();

    return (
      <g clipPath={`url(#${id})`}>
        <clipPath id={id}>{shapeElement(shape)}</clipPath>
        {inner}
      </g>
    );
  }, children);
}

/** Draws `drawable` in its node's local space. */
export function drawableElement(drawable: SceneDrawable, ids: SvgIds): ReactNode {
  switch (drawable.type) {
    case "fill": {
      const style: CSSProperties | undefined =
        drawable.blendMode && drawable.blendMode !== "normal"
          ? { mixBlendMode: drawable.blendMode }
          : undefined;

      if (drawable.paint.type === "pattern") {
        return clipped(
          [...(drawable.clips ?? []), drawable.shape, { type: "rect", rect: drawable.paint.area }],
          ids,
          <g style={style}>{patternTiles(drawable.paint, ids)}</g>,
        );
      }

      const { value, defs } = paintValue(drawable.paint, ids);

      return clipped(
        drawable.clips,
        ids,
        <>
          {defs && <defs>{defs}</defs>}
          {shapeElement(drawable.shape, { fill: value, style })}
        </>,
      );
    }
    case "stroke": {
      const { value, defs } = paintValue(drawable.paint, ids);

      return clipped(
        drawable.clips,
        ids,
        <>
          {defs && <defs>{defs}</defs>}
          {shapeElement(drawable.shape, {
            fill: "none",
            stroke: value,
            ...strokeProps(drawable.stroke),
          })}
        </>,
      );
    }
    case "shadow": {
      const filter = ids.next();
      const visible = ids.next();
      const color = cssColor(drawable.color);
      const paint = drawable.stroke
        ? { fill: "none", stroke: color, ...strokeProps(drawable.stroke) }
        : { fill: color };

      return (
        <g>
          <defs>
            <filter id={filter} x="-50%" y="-50%" width="200%" height="200%">
              <feGaussianBlur stdDeviation={drawable.blur} />
            </filter>
            <mask
              id={visible}
              maskUnits="userSpaceOnUse"
              x="-10000"
              y="-10000"
              width="20000"
              height="20000"
            >
              {drawable.visible === "outside" ? (
                <>
                  <rect x="-10000" y="-10000" width="20000" height="20000" fill="white" />
                  {shapeElement(drawable.box, { fill: "black" })}
                </>
              ) : (
                shapeElement(drawable.box, { fill: "white" })
              )}
            </mask>
          </defs>
          <g mask={`url(#${visible})`}>
            <g
              filter={drawable.blur > 0 ? `url(#${filter})` : undefined}
              transform={`translate(${drawable.offset.x} ${drawable.offset.y})`}
            >
              {shapeElement(drawable.shape, paint)}
            </g>
          </g>
        </g>
      );
    }
    case "glyphs": {
      const { run } = drawable;
      const { value, defs } = paintValue(drawable.paint, ids);
      const filter = ids.next();
      const paint = drawable.stroke
        ? { fill: "none", stroke: value, ...strokeProps(drawable.stroke) }
        : { fill: value };

      return (
        <g
          transform={`translate(${run.x + drawable.offset.x} ${run.y + drawable.offset.y})${run.transform ? ` ${matrixCss(run.transform)}` : ""}`}
        >
          <defs>
            {defs}
            {drawable.blur > 0 && (
              <filter id={filter} x="-50%" y="-50%" width="200%" height="200%">
                <feGaussianBlur stdDeviation={drawable.blur} />
              </filter>
            )}
          </defs>
          <path
            d={run.outline}
            filter={drawable.blur > 0 ? `url(#${filter})` : undefined}
            {...paint}
          />
        </g>
      );
    }
    case "image": {
      const id = ids.next();

      return (
        <g clipPath={`url(#${id})`}>
          <clipPath id={id}>{shapeElement(drawable.clip)}</clipPath>
          <image href={drawable.image.src} {...drawable.rect} preserveAspectRatio="none" />
        </g>
      );
    }
    case "masked": {
      const id = ids.next();

      return (
        <g>
          <mask id={id} style={{ maskType: "alpha" }}>
            {drawable.mask.map((inner, index) => (
              <g key={index}>{drawableElement(inner, ids)}</g>
            ))}
          </mask>
          <g mask={`url(#${id})`}>
            {drawable.content.map((inner, index) => (
              <g key={index}>{drawableElement(inner, ids)}</g>
            ))}
          </g>
        </g>
      );
    }
    case "group":
      return (
        <g opacity={drawable.opacity}>
          {drawable.drawables.map((inner, index) => (
            <g key={index}>{drawableElement(inner, ids)}</g>
          ))}
        </g>
      );
  }
}

type Frame = { wrap: (children: ReactNode[]) => ReactNode; children: ReactNode[] };

/**
 * The first `count` steps of `scene`, nested the way the steps open and close groups and clips.
 * Each step keeps its key as later steps arrive, so only new steps mount.
 */
export function sceneSteps(
  scene: PaintScene,
  count: number,
  ids: SvgIds,
  /** Fades each drawn step in, the later ones this many milliseconds after the one before. */
  stagger?: number,
): ReactNode {
  const root: Frame = { wrap: (children) => children, children: [] };
  const frames: Frame[] = [root];
  const top = () => frames[frames.length - 1] ?? root;
  const close = () => {
    const frame = frames.pop();
    if (frame) top().children.push(frame.wrap(frame.children));
  };

  scene.steps.slice(0, count).forEach((step, index) => {
    const node = scene.nodes[step.node];

    switch (step.type) {
      case "draw":
        top().children.push(
          <g
            key={index}
            className={stagger === undefined ? undefined : "paint-in"}
            style={stagger ? { animationDelay: `${index * stagger}ms` } : undefined}
            transform={node && matrixCss(node.transform)}
          >
            {step.drawables.map((drawable, at) => (
              <g key={at}>{drawableElement(drawable, ids)}</g>
            ))}
          </g>,
        );
        break;
      case "begin-group": {
        const style: CSSProperties = { isolation: "isolate" };

        if (step.blendMode !== "normal") style.mixBlendMode = step.blendMode;
        frames.push({
          wrap: (children) => (
            <g key={index} opacity={step.opacity} style={style}>
              {children}
            </g>
          ),
          children: [],
        });
        break;
      }
      case "begin-clip": {
        const id = ids.next();
        frames.push({
          wrap: (children) => (
            <g key={index} clipPath={`url(#${id})`}>
              <clipPath id={id}>
                <g transform={node && matrixCss(node.transform)}>{shapeElement(step.clip)}</g>
              </clipPath>
              {children}
            </g>
          ),
          children: [],
        });
        break;
      }
      case "end-group":
      case "end-clip":
        close();
        break;
    }
  });

  while (frames.length > 1) close();

  return root.children;
}
