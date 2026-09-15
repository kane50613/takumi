import type { CssInput, Node } from "@takumi-rs/helpers";

export type ByteBuf = Uint8Array | ArrayBuffer | Buffer;

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

export type Font = FontDetails | ByteBuf;

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

export type ImageSource = {
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
  images?: ImageSource[];
  /** CSS to apply before layout. */
  css?: CssInput[];
  /** Per-render font stack: ordered family names used as the fallback chain. */
  fontFamilies?: string[];
  /** Default BCP-47 language tag applied to the root. */
  lang?: string;
};

/** A color as `[r, g, b, a]`, each `0..=255`. */
export type Rgba = [number, number, number, number];

/** A rectangle in the owning node's border-box space. */
export type PaintRect = { x: number; y: number; width: number; height: number };

/** Corner radii as `[x, y]` pairs: top-left, top-right, bottom-right, bottom-left. */
export type Radii = [[number, number], [number, number], [number, number], [number, number]];

/** A 2D affine matrix as `[a, b, c, d, e, f]`. */
export type Matrix = [number, number, number, number, number, number];

/** Everything the backends paint for a node tree, in paint order. */
export type PaintTree = {
  /** Canvas width in device pixels. */
  width: number;
  /** Canvas height in device pixels. */
  height: number;
  /** Font instances the runs reference by index. */
  fonts: PaintFont[];
  root: PaintNode;
};

/** A font instance a run was shaped with. */
export type PaintFont = {
  /** The family the face was registered under; absent for a face the registry cannot name. */
  family?: string;
  /** Index of the face within its collection. */
  index: number;
  /** Weight class, `wght` applied when the face is variable. */
  weight: number;
  style: "normal" | "italic" | "oblique";
  /** Width as a percentage of normal. */
  width: number;
  variations: { tag: string; value: number }[];
  /** Stroke width in px for synthetic bold. */
  syntheticBold?: number;
  /** Synthetic oblique angle in degrees. */
  syntheticSkew?: number;
};

/**
 * One painted box: a compositing group whose `opacity`, `clip`, and `blendMode` apply to
 * everything inside it. Paints in order: `box`, `image`, `inlineBackgrounds`, `runs`,
 * `children`, then `box.outline`.
 */
export type PaintNode = {
  /** The node this box came from; absent for an anonymous box. */
  source?: PaintSource;
  /** Border-box width in device pixels. */
  width: number;
  /** Border-box height in device pixels. */
  height: number;
  /** Absolute transform placing the border box on the canvas. */
  transform: Matrix;
  opacity: number;
  /** `mix-blend-mode` other than `normal`. */
  blendMode?: string;
  /** Whether `isolation: isolate` applies. */
  isolate: boolean;
  /** Overflow clip applied to the children. */
  clip?: PaintClip;
  /** Box decorations, when the box paints any. */
  box?: PaintBox;
  image?: PaintImage;
  /** `text-shadow` layers under every run, later-listed shadows lowest. */
  textShadows?: PaintShadow[];
  /** Inline-span backgrounds, one rounded rect per line, outer spans first. */
  inlineBackgrounds?: PaintInlineBackground[];
  /** Shaped text runs in visual order. */
  runs?: PaintRun[];
  /** Effects the tree carries as CSS text instead of resolving. */
  unresolved?: PaintUnresolved;
  /** Boxes painted after this one, in paint order. */
  children?: PaintNode[];
};

export type PaintSource = {
  /** Child-index path from the input root. */
  path: number[];
  id?: string;
  tagName?: string;
  className?: string;
};

export type PaintClip = {
  rect: PaintRect;
  radii: Radii;
  /** Whether the horizontal axis clips. */
  x: boolean;
  /** Whether the vertical axis clips. */
  y: boolean;
};

export type PaintBox = {
  background: PaintBackground;
  border: PaintBorder;
  shadows: { inset: PaintShadow[]; outer: PaintShadow[] };
  /** The outline, painted after the children. */
  outline?: PaintOutline;
};

export type PaintBackground = {
  /** `background-color`, when visible. */
  color?: Rgba;
  /** `background-clip`; `text` means the background fills the glyphs instead of the box. */
  clip: string;
  /** Image layers, bottom to top. */
  layers?: PaintBackgroundLayer[];
};

export type PaintBackgroundLayer = {
  fill: PaintFill;
  /** Where the tiles land, in border-box space. */
  tiles: { xs: number[]; ys: number[]; width: number; height: number };
  blendMode: string;
};

export type PaintGradientStop = { color: Rgba; position: number };

export type PaintFill =
  | {
      kind: "linear";
      css: string;
      repeating: boolean;
      dirX: number;
      dirY: number;
      /** Length of the gradient axis in px. */
      axisLength: number;
      /** Stops in axis px from the axis start. */
      stops: PaintGradientStop[];
    }
  | {
      kind: "radial";
      css: string;
      repeating: boolean;
      cx: number;
      cy: number;
      radiusX: number;
      radiusY: number;
      /** Stops in px from the center along the radius. */
      stops: PaintGradientStop[];
    }
  | { kind: "conic"; css: string }
  | { kind: "image"; src?: string };

/** Sides ordered top, right, bottom, left. */
export type PaintBorder = {
  widths: [number, number, number, number];
  colors: [Rgba, Rgba, Rgba, Rgba];
  styles: [string, string, string, string];
  radii: Radii;
};

export type PaintShadow = {
  offsetX: number;
  offsetY: number;
  blur: number;
  spread: number;
  color: Rgba;
};

export type PaintOutline = {
  width: number;
  color: Rgba;
  style: string;
  /** Gap between the border edge and the outline. */
  offset: number;
};

export type PaintImage = {
  /** The image URL, when the source was one. */
  src?: string;
  /** The content box the image is placed in and clipped to. */
  content: PaintRect;
  /** Where the whole image draws after `object-fit` and `object-position`. */
  placement: PaintRect;
};

export type PaintRun = {
  text: string;
  /** Start of the run's baseline, x. */
  x: number;
  /** The run's baseline, y. */
  y: number;
  /** Advance of the run. */
  width: number;
  ascent: number;
  descent: number;
  /** Index into `PaintTree.fonts`. */
  font: number;
  fontSize: number;
  color: Rgba;
  opacity: number;
  /** A transform on top of the node's, when text-fit scales the line. */
  transform?: Matrix;
  /** Glyphs relative to the run origin. */
  glyphs: { id: number; x: number; y: number }[];
  decorations?: PaintDecoration[];
  stroke?: { color: Rgba; width: number };
  /** UTF-8 byte range of the text within the node's inline text. */
  textRange: [number, number];
  /** The inline span the run came from. */
  span?: number;
};

export type PaintDecoration = {
  line: "underline" | "overline" | "line-through";
  transform: Matrix;
  width: number;
  height: number;
  color: Rgba;
};

export type PaintInlineBackground = {
  rect: PaintRect;
  radii: Radii;
  color: Rgba;
  opacity: number;
};

export type PaintUnresolved = {
  filter?: string;
  backdropFilter?: string;
  mask?: string;
  clipPath?: string;
};
