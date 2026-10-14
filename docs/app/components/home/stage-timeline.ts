/** The three hammer impact points along the 0..1 forge timeline. */
const IMPACTS = [0.21, 0.49, 0.77] as const;

/** Settled scroll progress stops for [Anvil, Layout, Text, Paint]. */
export const STRIKE_STOPS = [0, 0.38, 0.65, 0.93] as const;

/** The progress at which each phase becomes active. */
export const STEP_STARTS = [0, 0.19, 0.47, 0.74] as const;

/** 0 before `from`, 1 after `to`, eased in and out between. */
function ramp(progress: number, [from, to]: readonly [number, number]) {
  const t = Math.min(Math.max((progress - from) / (to - from), 0), 1);

  return t * t * (3 - 2 * t);
}

/** Fast whip-down easing for the hammer downswing. */
function strikeDown(progress: number, from: number, to: number) {
  const t = Math.min(Math.max((progress - from) / (to - from), 0), 1);

  return t * t * t;
}

/**
 * Hammer rotation in degrees:
 * -11° is poised in full view above the anvil, -14° is cocked back, and 0° is flat on the anvil face.
 */
export function hammerAngle(progress: number): number {
  if (progress < 0.04) {
    return -11;
  }
  // Strike 1: wind up (0.04 -> 0.14), slam down (0.14 -> 0.21), rebound (0.21 -> 0.38)
  if (progress < 0.14) {
    return -11 - 3 * ramp(progress, [0.04, 0.14]);
  }
  if (progress < 0.21) {
    return -14 + 14 * strikeDown(progress, 0.14, 0.21);
  }
  if (progress < 0.38) {
    return -11 * ramp(progress, [0.21, 0.29]);
  }
  // Strike 2: wind up (0.38 -> 0.44), slam down (0.44 -> 0.49), rebound (0.49 -> 0.66)
  if (progress < 0.44) {
    return -11 - 3 * ramp(progress, [0.38, 0.44]);
  }
  if (progress < 0.49) {
    return -14 + 14 * strikeDown(progress, 0.44, 0.49);
  }
  if (progress < 0.66) {
    return -11 * ramp(progress, [0.49, 0.56]);
  }
  // Strike 3: wind up (0.66 -> 0.72), slam down (0.72 -> 0.77), lift clear (0.77 -> 1.0)
  if (progress < 0.72) {
    return -11 - 3.5 * ramp(progress, [0.66, 0.72]);
  }
  if (progress < 0.77) {
    return -14.5 + 14.5 * strikeDown(progress, 0.72, 0.77);
  }
  return -11.5 * ramp(progress, [0.77, 0.85]);
}

/** Where `progress` sits inside strike `step`, from -0.5 at its start to 0.5 at the next one. */
export function stepDrift(progress: number, step: number) {
  const from = STEP_STARTS[step] ?? 0;
  const to = STEP_STARTS[step + 1] ?? 1;

  return Math.min(Math.max((progress - from) / (to - from), 0), 1) - 0.5;
}

/**
 * Backdrop word `index`'s `font-stretch` (%) and `font-weight`: it widens and thickens across its
 * strike, starting partway for the opening word, and each blow squashes it back.
 */
export function wordAxes(progress: number, index: number) {
  const from = index === 0 ? -0.35 : (STEP_STARTS[index] ?? 0);
  const to = STEP_STARTS[index + 1] ?? 1;
  const grown = ramp(progress, [from, to]);

  return {
    stretch: Math.max(50, 55 + 85 * grown - 30 * strikeImpulse(progress)),
    weight: 700 + 200 * grown,
  };
}

/** Sharp impulse (0..1) right as the hammer strikes the anvil at any of the three blows. */
export function strikeImpulse(progress: number): number {
  let maxImpulse = 0;

  for (const impact of IMPACTS) {
    const delta = progress - impact;

    if (delta >= -0.015 && delta <= 0.085) {
      const normalized = delta < 0 ? (delta + 0.015) / 0.015 : 1 - delta / 0.085;
      const pulse = Math.max(0, Math.min(1, normalized));

      if (pulse > maxImpulse) {
        maxImpulse = pulse * pulse;
      }
    }
  }

  return maxImpulse;
}

/** 0..1 explosion factor for Strike I: Layout boxes. */
export function layoutSpread(progress: number): number {
  return ramp(progress, [0.195, 0.27]) * (1 - ramp(progress, [0.44, 0.51]));
}

/** 0..1 explosion factor for Strike II: Shaped glyph outlines and metrics. */
export function textSpread(progress: number): number {
  return ramp(progress, [0.475, 0.55]) * (1 - ramp(progress, [0.71, 0.78]));
}

/** 0..1 explosion factor for Strike III: Ordered paint draw steps. */
export function paintSpread(progress: number): number {
  return ramp(progress, [0.755, 0.84]);
}
