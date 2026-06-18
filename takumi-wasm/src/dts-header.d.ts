import type { Node } from "@takumi-rs/helpers";
import type { Properties } from "csstype";

export { ContainerNode, ImageNode, Node, NodeMetadata, TextNode } from "@takumi-rs/helpers";

export type ByteBuf = Uint8Array | ArrayBuffer | Buffer;

export type KeyframesMap = Record<string, Record<string, Properties>>;
export type KeyframesRuleList = {
  name: string;
  keyframes: {
    offsets: number[];
    declarations: Record<string, Properties>;
  }[];
}[];
export type Keyframes = KeyframesMap | KeyframesRuleList;

export type RenderOptions = {
  /**
   * The width of the image. If not provided, the width will be automatically calculated based on the content.
   */
  width?: number;
  /**
   * The height of the image. If not provided, the height will be automatically calculated based on the content.
   */
  height?: number;
  /**
   * The format of the image.
   * @default "png"
   */
  format?: "png" | "jpeg" | "webp" | "ico" | "raw";
  /**
   * The quality of JPEG format (0-100).
   */
  quality?: number;
  /**
   * The resources fetched externally. You should collect the fetch tasks first using `extractResourceUrls` and then pass the resources here.
   */
  images?: ImageSource[];
  /**
   * CSS stylesheets to apply before rendering.
   */
  stylesheets?: string[];
  /**
   * Structured keyframes to register alongside stylesheets.
   */
  keyframes?: Keyframes;
  /**
   * Whether to draw debug borders.
   */
  drawDebugBorder?: boolean;
  /**
   * Defines the ratio resolution of the image to the physical pixels.
   * @default 1.0
   */
  devicePixelRatio?: number;
  /**
   * The animation timeline time in milliseconds.
   */
  timeMs?: number;
  /**
   * The output dithering algorithm.
   * @default "none"
   */
  dithering?: "none" | "ordered-bayer" | "floyd-steinberg";
  /**
   * Per-render font fallback chain (family names in order, e.g. from `registerFonts`).
   * Defaults to all registered families in registration order.
   */
  fonts?: string[];
};

export type RenderAnimationOptions = {
  scenes: AnimationSceneSource[];
  width: number;
  height: number;
  format?: "webp" | "apng" | "gif";
  /**
   * The quality of WebP format (0-100). Ignored for APNG and GIF.
   */
  quality?: number;
  /**
   * The resources fetched externally. You should collect the fetch tasks first using `extractResourceUrls` and then pass the resources here.
   */
  images?: ImageSource[];
  drawDebugBorder?: boolean;
  /**
   * CSS stylesheets to apply before rendering.
   */
  stylesheets?: string[];
  /**
   * Defines the ratio resolution of the image to the physical pixels.
   * @default 1.0
   */
  devicePixelRatio?: number;
  /**
   * Frames per second for timeline sampling.
   */
  fps: number;
};

export type EncodeFramesOptions = {
  width: number;
  height: number;
  format?: "webp" | "apng" | "gif";
  /**
   * The quality of WebP format (0-100). Ignored for APNG and GIF.
   */
  quality?: number;
  /**
   * The resources fetched externally. You should collect the fetch tasks first using `extractResourceUrls` and then pass the resources here.
   */
  images?: ImageSource[];
  drawDebugBorder?: boolean;
  /**
   * CSS stylesheets to apply before rendering.
   */
  stylesheets?: string[];
  /**
   * Defines the ratio resolution of the image to the physical pixels.
   * @default 1.0
   */
  devicePixelRatio?: number;
};

export type FontDetails = {
  name?: string;
  data: ByteBuf;
  weight?: number;
  style?: "normal" | "italic" | "oblique" | `oblique ${number}deg` | (string & {});
};

export type ImageSource = {
  src: string;
  data: ByteBuf;
};

export type KeyframeRule = {
  offsets: number[];
  declarations: Record<string, Properties>;
};

export type KeyframesRule = {
  name: string;
  keyframes: KeyframeRule[];
};

export type Font = FontDetails | ByteBuf;

export type ConstructRendererOptions = {
  /**
   * The fonts being used.
   */
  fonts?: Font[];
  /**
   * Whether to load the default fonts.
   * If `fonts` are provided, this will be `false` by default.
   */
  loadDefaultFonts?: boolean;
};

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

export type MeasuredTextRun = {
  text: string;
  x: number;
  y: number;
  width: number;
  height: number;
};

export type MeasuredNode = {
  width: number;
  height: number;
  transform: [number, number, number, number, number, number];
  children: MeasuredNode[];
  runs: MeasuredTextRun[];
};

export type AnimationFrameSource = {
  node: Node;
  durationMs: number;
};

export type AnimationSceneSource = {
  node: Node;
  durationMs: number;
};
