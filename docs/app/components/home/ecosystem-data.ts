/** What the server hands the homepage scene, and where the card sits in the viewport. */

export const CARD_LAYOUT = { width: 1200, height: 630 };

/** At or above this width the text sits left of the card; below it the card sits above the text. */
export const WIDE_MIN = 1024;

export type EcosystemData = {
  card: { image: string; motion: string; svg: string };
  /** Lines of the card's JSX source. */
  source: string[];
  boxes: {
    label: string;
    depth: number;
    x: number;
    y: number;
    width: number;
    height: number;
    leaf: boolean;
  }[];
  /** One label per paint layer, such as `steps[3] h1`. */
  steps: string[];
  outputs: { title: string; detail: string }[];
  runtimes: { name: string; detail: string }[];
};

export type Palette = { foreground: string; muted: string; accent: string };

/**
 * The card's center, measured from the viewport's center with y up, and its width, in CSS
 * pixels. The hero poster's CSS in ecosystem.tsx mirrors this; keep them in sync.
 */
export function cardFrame(width: number, height: number) {
  if (width >= WIDE_MIN) {
    return { x: width * 0.2, y: 0, width: Math.min(width * 0.46, 720) };
  }

  return { x: 0, y: height * 0.18, width: width * 0.86 };
}
