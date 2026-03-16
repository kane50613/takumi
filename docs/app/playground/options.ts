declare type PlaygroundOptions = {
  /**
   * @description width of the render viewport.
   */
  width?: number;
  /**
   * @description height of the render viewport.
   */
  height?: number;
  /**
   * @description format to render.
   * @default png
   */
  format?: "png" | "jpeg" | "webp";
  /**
   * @description quality of jpeg format (0-100).
   * @default 75
   */
  quality?: number;
  /**
   * @description device pixel ratio.
   * @default 1.0
   */
  devicePixelRatio?: number;
  /**
   * @description CSS stylesheets applied before rendering.
   */
  stylesheets?: string[];
  /**
   * @description timeline animation output. When present, the playground renders an animated image instead of a single frame.
   */
  animation?: {
    /**
     * @description total timeline duration in milliseconds.
     */
    durationMs: number;
    /**
     * @description frames per second used to sample keyframes.
     * @default 30
     */
    fps?: number;
    /**
     * @description animation output format.
     * @default webp
     */
    format?: "webp" | "apng" | "gif";
  };
  /**
   * @description emoji style to use.
   * @default twemoji
   */
  emoji?: "twemoji" | "blobmoji" | "noto" | "openmoji";
};
