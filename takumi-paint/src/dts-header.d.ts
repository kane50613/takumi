import type { CssInput, Node } from "@takumi-rs/helpers";

export type ByteBuf = Uint8Array | ArrayBuffer;

/** Cache policy for a decoded image. Defaults to `"auto"`. */
export type ImageCacheMode = "auto" | "none";

export type FontDetails = {
  name?: string;
  data: ByteBuf;
  weight?: number;
  style?: "normal" | "italic" | "oblique" | `oblique ${number}deg` | (string & {});
  /**
   * Logical family this font is a coverage subset of. Subsets sharing a
   * `subsetOf` are kept as distinct families and `font-family: {subsetOf}`
   * expands to all of them, so each script routes to the subset that covers it.
   */
  subsetOf?: string;
  /**
   * Where this subset sits in its group's fallback order; lowest is tried first, and equal
   * ranks order by family name.
   */
  subsetRank?: number;
  /** CSS generic family keyword this font resolves for. */
  generic?: string;
};

export type FontInput = FontDetails | ByteBuf;

export type RegisteredFace = {
  weight: number;
  style: string;
  width: number;
  index: number;
};

export type RegisteredFamily = {
  name: string;
  faces: RegisteredFace[];
};

export type ImageInput = {
  src: string;
  data: ByteBuf;
  /** Cache policy for the decoded image. Defaults to `"auto"`. */
  cache?: ImageCacheMode;
};

export type PaintOptions = {
  /** Canvas width in device pixels. Omit to size the canvas to the content. */
  width?: number;
  /** Canvas height in device pixels. Omit to size the canvas to the content. */
  height?: number;
  /** Device pixels per CSS px; scales every CSS length. @default 1 */
  devicePixelRatio?: number;
  /** Pre-fetched images keyed by URL. */
  images?: ImageInput[];
  /** CSS to apply before layout. */
  css?: CssInput[];
  /** Per-render font stack: ordered family names used as the fallback chain. */
  fontFamilies?: string[];
  /** Default BCP-47 language tag applied to the root. */
  lang?: string;
};

/** `[r, g, b, a]`, each `0..=255`, in sRGB. */
export type Rgba = [number, number, number, number];
/** `[a, b, c, d, e, f]`, the order `CanvasRenderingContext2D.setTransform` takes. */
export type Matrix = [number, number, number, number, number, number];
export type Point = { readonly x: number; readonly y: number };
export type Rect = {
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly height: number;
};
/** Horizontal and vertical radius of each corner. */
export type CornerRadii = {
  readonly topLeft: Point;
  readonly topRight: Point;
  readonly bottomRight: Point;
  readonly bottomLeft: Point;
};

export type Shape =
  | { readonly type: "rect"; readonly rect: Rect }
  | { readonly type: "rounded-rect"; readonly rect: Rect; readonly radii: CornerRadii }
  /** SVG path data, the string `new Path2D()` takes. */
  | { readonly type: "path"; readonly d: string; readonly fillRule: "nonzero" | "evenodd" };

/**
 * A color stop, `offset` in `0..=1`. Interpolate linearly in sRGB between neighbors: takumi adds
 * stops wherever the CSS interpolation color space, such as the default Oklab, would differ, and
 * unrolls repeating gradients over the area they cover.
 */
export type ColorStop = { readonly offset: number; readonly color: Rgba };

export type Gradient =
  | {
      readonly type: "linear-gradient";
      readonly start: Point;
      readonly end: Point;
      readonly stops: readonly ColorStop[];
    }
  /** Elliptical when `radiusX !== radiusY`: scale the y axis by `radiusY / radiusX` about `center`. */
  | {
      readonly type: "radial-gradient";
      readonly center: Point;
      readonly radiusX: number;
      readonly radiusY: number;
      readonly stops: readonly ColorStop[];
    }
  /** `startAngle` in radians, clockwise from the positive x axis, as `createConicGradient` takes it. */
  | {
      readonly type: "conic-gradient";
      readonly center: Point;
      readonly startAngle: number;
      readonly stops: readonly ColorStop[];
    };

/** A decoded image. */
export type ImageSource = {
  /** The `src` the document referenced, or a `data:` URL for image bytes passed inline. */
  readonly src: string;
  readonly width: number;
  readonly height: number;
};

/** `pixelated` means nearest-neighbor sampling. */
export type Sampling = "smooth" | "pixelated";

export type ImagePaint = {
  readonly type: "image";
  readonly image: ImageSource;
  readonly sampling: Sampling;
};

export type Paint =
  | { readonly type: "color"; readonly color: Rgba }
  | Gradient
  | ImagePaint
  /** A tile repeated at every `x` and `y` pair: CSS background and mask layers. */
  | {
      readonly type: "pattern";
      readonly tile: Gradient | ImagePaint;
      readonly tileWidth: number;
      readonly tileHeight: number;
      readonly x: readonly number[];
      readonly y: readonly number[];
    };

export type Stroke = {
  readonly width: number;
  /** Alternating dash and gap lengths; absent for a solid line. */
  readonly dash?: readonly number[];
  readonly cap: "butt" | "round";
  readonly join: "miter" | "round" | "bevel";
};

/** What a drawable is for. Only exporters that rebuild editable objects need it. */
export type Role =
  | "background"
  | "border"
  | "box-shadow"
  | "outline"
  | "image"
  | "text"
  | "text-shadow"
  | "text-stroke"
  | "text-decoration"
  | "inline-background";

/** Separable and non-separable blend modes; all but `normal` match Canvas `globalCompositeOperation`. */
export type BlendMode =
  | "normal"
  | "multiply"
  | "screen"
  | "overlay"
  | "darken"
  | "lighten"
  | "color-dodge"
  | "color-burn"
  | "hard-light"
  | "soft-light"
  | "difference"
  | "exclusion"
  | "hue"
  | "saturation"
  | "color"
  | "luminosity"
  | "plus-lighter";

/** Something to draw, in the owning node's local space, with glyph runs named by index. */
export type RawDrawable<Run = number> =
  /** With `blendMode`, blends with what the enclosing group already holds, as `background-blend-mode` does. */
  | {
      readonly type: "fill";
      readonly role: Role;
      readonly shape: Shape;
      readonly paint: Paint;
      readonly blendMode?: BlendMode;
      /** Shapes it is clipped to, all at once. */
      readonly clips?: readonly Shape[];
    }
  | {
      readonly type: "stroke";
      readonly role: Role;
      readonly shape: Shape;
      readonly stroke: Stroke;
      readonly paint: Paint;
      /** Shapes it is clipped to, all at once. */
      readonly clips?: readonly Shape[];
    }
  /**
   * A blurred copy of `shape`, moved by `offset`, visible only on one side of `box`: outside for
   * an outer box shadow, inside for an inset one. `blur` is the Gaussian's standard deviation.
   */
  | {
      readonly type: "shadow";
      readonly role: Role;
      readonly shape: Shape;
      readonly offset: Point;
      readonly blur: number;
      readonly color: Rgba;
      readonly visible: "outside" | "inside";
      readonly box: Shape;
    }
  /**
   * A run's glyphs filled with `paint`, which sits in the node's space. With `blur`, a text
   * shadow. With `stroke`, the outlines are stroked instead of filled.
   */
  | {
      readonly type: "glyphs";
      readonly role: Role;
      readonly run: Run;
      readonly paint: Paint;
      readonly offset: Point;
      readonly blur: number;
      readonly stroke?: Stroke;
    }
  | {
      readonly type: "image";
      readonly role: Role;
      readonly image: ImageSource;
      readonly rect: Rect;
      readonly clip: Shape;
      readonly sampling: Sampling;
    };

export type Filter =
  /** Gaussian blur; `radius` is the standard deviation in px. */
  | { readonly type: "blur"; readonly radius: number }
  /** A 4×5 row-major matrix, as SVG `feColorMatrix type="matrix"`. Brightness, contrast, grayscale, hue-rotate, invert, opacity, saturate, and sepia all become one. */
  | { readonly type: "color-matrix"; readonly matrix: readonly number[] }
  | {
      readonly type: "drop-shadow";
      readonly offset: Point;
      readonly blur: number;
      readonly color: Rgba;
    }
  /** A filter takumi cannot resolve, such as `url(#svg-filter)`. */
  | { readonly type: "unsupported"; readonly css: string };

/** How a node composites, its mask's glyph runs named by index. */
export type RawEffects<Drawable = RawDrawable> = {
  readonly opacity: number;
  readonly blendMode: BlendMode;
  readonly isolation: boolean;
  readonly filters: readonly Filter[];
  /** Filters applied to what is already drawn behind the node, before the node draws. */
  readonly backdropFilters: readonly Filter[];
  /** The shape the filtered backdrop shows through: the node's border box. */
  readonly backdropClip?: Shape;
  readonly clip?: Shape;
  /** Draw these into a layer; its alpha masks the node. */
  readonly mask?: readonly Drawable[];
};

export type ElementInfo = {
  readonly id?: string;
  readonly tagName?: string;
  readonly className?: string;
  /** Child-index path from the input root. */
  readonly path: readonly number[];
};

export type Glyph = { readonly id: number; readonly x: number; readonly y: number };

export type RawTextRun = {
  readonly text: string;
  readonly element?: ElementInfo;
  readonly x: number;
  readonly y: number;
  readonly width: number;
  readonly line: number;
  readonly ascent: number;
  readonly descent: number;
  readonly font: number;
  readonly fontSize: number;
  readonly lineHeight: number;
  readonly letterSpacing: number;
  readonly glyphs: readonly Glyph[];
  readonly outline: string;
  readonly transform?: Matrix;
};

export type RawFont = {
  readonly family?: string;
  readonly weight: number;
  readonly style: "normal" | "italic" | "oblique";
  readonly stretch: number;
  readonly variationSettings: readonly { readonly tag: string; readonly value: number }[];
  readonly faceIndex: number;
};

type RawNodeBase = {
  readonly id: number;
  readonly parent?: number;
  readonly element?: ElementInfo;
  readonly transform: Matrix;
  readonly width: number;
  readonly height: number;
  readonly bounds: Rect;
  readonly drawables: readonly RawDrawable[];
  readonly children: readonly number[];
};

export type RawPaintNode =
  | (RawNodeBase & {
      readonly type: "box";
      readonly contentBox: Rect;
      readonly outline: readonly RawDrawable[];
      readonly effects?: RawEffects;
      readonly overflowClip?: Shape;
    })
  | (RawNodeBase & {
      readonly type: "text";
      readonly textAlign: "left" | "right" | "center" | "justify";
      readonly runs: readonly RawTextRun[];
    })
  | (RawNodeBase & { readonly type: "image"; readonly image: ImageSource });

export type RawPaintStep =
  | { readonly type: "draw"; readonly node: number; readonly part: "drawables" | "outline" }
  | {
      readonly type: "begin-group" | "end-group" | "begin-clip" | "end-clip";
      readonly node: number;
    };

/** The tree as the wasm module serializes it, nodes and runs named by index. */
export type RawPaintTree = {
  readonly width: number;
  readonly height: number;
  readonly nodes: readonly RawPaintNode[];
  readonly fonts: readonly RawFont[];
  readonly steps: readonly RawPaintStep[];
};
