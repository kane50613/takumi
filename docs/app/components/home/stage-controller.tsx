"use client";

import { useEffect, useRef } from "react";
import {
  hammerAngle,
  layoutSpread,
  paintSpread,
  STEP_STARTS,
  stepDrift,
  STRIKE_STOPS,
  strikeImpulse,
  textSpread,
  wordAxes,
} from "./stage-timeline";

const FOLLOW_MS = 105;
const WIDE_MIN = 1024;
/** The forge's full composition, and the crop around hammer, card, and anvil narrower pages show. */
const FULL_VIEW_BOX = "200 -40 1440 940";
const COMPACT_VIEW_BOX = "420 -40 1000 900";
/** How far a forge part drifts over one strike: `base` units scaled by its `data-depth`. */
function depthDistance(element: HTMLElement | SVGElement, base: number) {
  return base * Number(element.dataset.depth ?? 1);
}

/** How much of a strike's spread the intact card takes to fade out, and its parts to fade in. */
const CARD_HANDOFF = 0.2;
const PARTS_HANDOFF = 0.35;

function partsOpacity(spread: number) {
  return Math.min(1, spread / PARTS_HANDOFF).toFixed(3);
}

/** Card-space gap between stacked paint planes, and how flat the isometric view squashes them. */
const PLANE_GAP = 105;
/** How much later in the spread each paint plane appears than the one below it. */
const PLANE_STAGGER = 0.1;
const ISO_SQUASH = 0.55;

/** The paint planes' view at `spread`, from flat to isometric, orbiting `10° × drift` off -45°. */
function isometricStack(spread: number, drift: number) {
  const angle = ((-45 + 10 * drift) * Math.PI) / 180;
  const cos = Math.cos(angle);
  const sin = Math.sin(angle);
  const a = 1 + (cos - 1) * spread;
  const b = ISO_SQUASH * sin * spread;
  const c = -sin * spread;
  const d = 1 + (ISO_SQUASH * cos - 1) * spread;

  return { spread, matrix: `matrix(${[a, b, c, d].map((v) => v.toFixed(4)).join(",")},0,0)` };
}

const GLIDE_MS = 1200;
/** Reader input that takes the scroll back from a glide. */
const GLIDE_INTERRUPTS = ["wheel", "touchstart", "keydown"] as const;
const STRIKE_LABELS = [
  "Strike the anvil",
  "Strike again · Text",
  "Strike again · Paint",
  "Quench into outputs",
] as const;

/**
 * Drives the homepage hammer-and-anvil forge from scroll or click/key strikes: each blow swings
 * the hammer onto the anvil and splits the component into layout boxes, glyph paths, and paint steps.
 */
export function StageController() {
  const anchorRef = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    const stage = anchorRef.current?.closest<HTMLElement>("[data-stage]");
    const view = stage?.querySelector<HTMLElement>("[data-stage-view]");
    const stepButtons = [
      ...(stage?.querySelectorAll<HTMLButtonElement>("[data-step-button]") ?? []),
    ];
    const strikeTriggers = [
      ...(stage?.querySelectorAll<HTMLElement>("[data-strike-trigger]") ?? []),
    ];
    const forge = stage?.querySelector<SVGSVGElement>("[data-forge]");
    const wide = matchMedia(`(min-width: ${WIDE_MIN}px)`);

    if (!stage || !view) {
      return;
    }

    const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;
    let frame = 0;
    let target = 0;
    let progress = 0;
    let last = performance.now();

    const getTravel = () => {
      const rect = stage.getBoundingClientRect();
      const pinnedTop = parseFloat(getComputedStyle(view).top) || 0;
      const travel = rect.height - view.offsetHeight;

      return { rect, pinnedTop, travel };
    };

    const measure = () => {
      const { rect, pinnedTop, travel } = getTravel();

      target = travel > 0 ? Math.min(Math.max((pinnedTop - rect.top) / travel, 0), 1) : 0;
    };

    let glide = 0;
    const stopGlide = () => {
      cancelAnimationFrame(glide);
      document.documentElement.style.scrollSnapType = "";
    };

    /** Scrolls to `top` slower than the browser's smooth scroll, so each strike plays out. */
    const glideTo = (top: number) => {
      stopGlide();
      if (reduceMotion) {
        window.scrollTo(0, top);
        return;
      }

      const from = window.scrollY;
      const start = performance.now();

      // Snap points would pull each frame's scroll toward them mid-glide.
      document.documentElement.style.scrollSnapType = "none";
      const tick = (now: number) => {
        const t = Math.min(1, (now - start) / GLIDE_MS);
        const eased = t < 0.5 ? 4 * t ** 3 : 1 - (-2 * t + 2) ** 3 / 2;

        window.scrollTo(0, from + (top - from) * eased);
        if (t < 1) {
          glide = requestAnimationFrame(tick);
        } else {
          stopGlide();
        }
      };

      glide = requestAnimationFrame(tick);
    };

    const scrollToProgress = (nextProgress: number) => {
      const { rect, pinnedTop, travel } = getTravel();

      if (travel <= 0) {
        return;
      }
      const stageTopInDoc = rect.top + window.scrollY - pinnedTop;

      glideTo(stageTopInDoc + nextProgress * travel);
    };

    const onStrike = () => {
      const nextStop = STRIKE_STOPS.find((stop) => stop > target + 0.08);

      if (nextStop !== undefined) {
        scrollToProgress(nextStop);
        return;
      }

      const outputsSection = document.getElementById("outputs");

      if (outputsSection) {
        glideTo(outputsSection.getBoundingClientRect().top + window.scrollY);
      }
    };

    const scene = new ForgeScene(stage, stepButtons);
    const apply = (p: number) => scene.render(forgeFrame(p, reduceMotion));

    const draw = (now: number) => {
      frame = 0;

      const follow = reduceMotion ? 1 : 1 - Math.exp(-(now - last) / FOLLOW_MS);

      last = now;
      progress += (target - progress) * follow;
      if (Math.abs(target - progress) < 0.0005) {
        progress = target;
      }
      apply(progress);
      if (progress !== target) {
        frame = requestAnimationFrame(draw);
      }
    };

    const wake = () => {
      measure();
      if (frame === 0) {
        last = performance.now();
        frame = requestAnimationFrame(draw);
      }
    };

    const onStepClick = (event: Event) => {
      const button = event.currentTarget as HTMLButtonElement;
      const index = Number(button.dataset.stepIndex ?? 0);
      const stop = STRIKE_STOPS[index] ?? 0;

      scrollToProgress(stop);
    };

    for (const trigger of strikeTriggers) {
      trigger.addEventListener("click", onStrike);
    }
    for (const button of stepButtons) {
      button.addEventListener("click", onStepClick);
    }

    const resizeObserver = new ResizeObserver(wake);

    const frameForge = () => {
      forge?.setAttribute("viewBox", wide.matches ? FULL_VIEW_BOX : COMPACT_VIEW_BOX);
    };

    frameForge();
    measure();
    progress = target;
    apply(progress);
    addEventListener("scroll", wake, { passive: true });
    for (const type of GLIDE_INTERRUPTS) {
      addEventListener(type, stopGlide, { passive: true });
    }
    wide.addEventListener("change", frameForge);
    resizeObserver.observe(view);

    return () => {
      for (const trigger of strikeTriggers) {
        trigger.removeEventListener("click", onStrike);
      }
      for (const button of stepButtons) {
        button.removeEventListener("click", onStepClick);
      }
      cancelAnimationFrame(frame);
      removeEventListener("scroll", wake);
      stopGlide();
      for (const type of GLIDE_INTERRUPTS) {
        removeEventListener(type, stopGlide);
      }
      wide.removeEventListener("change", frameForge);
      resizeObserver.disconnect();
    };
  }, []);

  return <span ref={anchorRef} hidden />;
}

/** Everything the forge shows at one scroll progress, settled per strike under reduced motion. */
type ForgeFrame = {
  progress: number;
  step: number;
  reduceMotion: boolean;
  angle: number;
  impulse: number;
  layout: number;
  text: number;
  paint: number;
};

function forgeFrame(progress: number, reduceMotion: boolean): ForgeFrame {
  const step = Math.max(
    0,
    STEP_STARTS.findLastIndex((start) => progress >= start),
  );

  if (reduceMotion) {
    return {
      progress,
      step,
      reduceMotion,
      angle: step === 0 ? -20 : -8,
      impulse: 0,
      layout: step === 1 ? 1 : 0,
      text: step === 2 ? 1 : 0,
      paint: step === 3 ? 1 : 0,
    };
  }

  return {
    progress,
    step,
    reduceMotion,
    angle: hammerAngle(progress),
    impulse: strikeImpulse(progress),
    layout: layoutSpread(progress),
    text: textSpread(progress),
    paint: paintSpread(progress),
  };
}

/** `-stepDrift` for the strike, or still under reduced motion. */
function strikeDrift(frame: ForgeFrame, step: number) {
  return frame.reduceMotion ? 0 : -stepDrift(frame.progress, step);
}

/** Shows or hides a layer of parts at `spread`, returning whether its parts need moving. */
function showLayer(layer: SVGGElement | null | undefined, spread: number, opacity: string) {
  if (layer) {
    layer.style.opacity = opacity;
    layer.style.visibility = spread > 0.01 ? "visible" : "hidden";
  }

  return spread > 0.01;
}

/** The forge's DOM, queried once, and how one frame of the timeline paints onto it. */
class ForgeScene {
  private readonly hammer;
  private readonly anvil;
  private readonly impactBurst;
  private readonly anvilCard;
  private readonly layoutLayer;
  private readonly textLayer;
  private readonly paintLayer;
  private readonly boxes;
  private readonly glyphs;
  private readonly planes;
  private readonly copyBlocks;
  private readonly words;
  private readonly backdropLayer;
  private readonly captionLines;
  private readonly strikeLabel;
  private activeStep = -1;

  constructor(
    stage: HTMLElement,
    private readonly stepButtons: HTMLButtonElement[],
  ) {
    this.hammer = stage.querySelector<SVGGElement>("[data-hammer]");
    this.anvil = stage.querySelector<SVGGElement>("[data-anvil]");
    this.impactBurst = stage.querySelector<SVGGElement>("[data-impact-burst]");
    this.anvilCard = stage.querySelector<SVGGElement>("[data-anvil-card]");
    this.layoutLayer = stage.querySelector<SVGGElement>("[data-layer='layout']");
    this.textLayer = stage.querySelector<SVGGElement>("[data-layer='text']");
    this.paintLayer = stage.querySelector<SVGGElement>("[data-layer='paint']");
    this.boxes = [...stage.querySelectorAll<SVGGElement>("[data-box]")];
    this.glyphs = [...stage.querySelectorAll<SVGGElement>("[data-glyph-run]")];
    this.planes = [...stage.querySelectorAll<SVGGElement>("[data-paint-plane]")];
    this.copyBlocks = [...stage.querySelectorAll<HTMLElement>("[data-stage-copy]")];
    this.words = [...stage.querySelectorAll<HTMLElement>("[data-stage-word]")];
    this.backdropLayer = stage.querySelector<HTMLElement>("[data-stage-backdrop]");
    this.captionLines = this.copyBlocks
      .map((block) => [...block.querySelectorAll<HTMLElement>("[data-stage-line]")])
      .filter((lines) => lines.length > 0);
    this.strikeLabel = stage.querySelector<HTMLElement>("[data-strike-label]");
  }

  render(frame: ForgeFrame) {
    this.strike(frame);
    this.layout(frame);
    this.text(frame);
    this.paint(frame);
    this.backdrop(frame);
    this.syncStep(frame.step);
  }

  /** The hammer, the anvil's jolt, the impact burst, and the intact card handing off to its parts. */
  private strike({ angle, impulse, layout, text, paint, reduceMotion }: ForgeFrame) {
    const intactOpacity = Math.max(0, 1 - Math.max(layout, text, paint) / CARD_HANDOFF);

    if (this.hammer) {
      this.hammer.style.transform = `rotate(${-angle.toFixed(2)}deg)`;
    }
    if (this.anvil) {
      this.anvil.style.transform = reduceMotion ? "" : `translateY(${(impulse * 12).toFixed(2)}px)`;
    }
    if (this.impactBurst) {
      this.impactBurst.style.opacity = impulse.toFixed(3);
      this.impactBurst.style.transform = `scale(${(0.72 + impulse * 0.62).toFixed(3)})`;
    }
    if (this.anvilCard) {
      this.anvilCard.style.opacity = intactOpacity.toFixed(3);
      this.anvilCard.style.transform = reduceMotion
        ? ""
        : `translateY(${(impulse * 9).toFixed(2)}px) scale(${(1 - 0.04 * (1 - intactOpacity)).toFixed(3)})`;
    }
  }

  private layout(frame: ForgeFrame) {
    const spread = frame.layout;

    if (!showLayer(this.layoutLayer, spread, partsOpacity(spread))) {
      return;
    }

    const drift = strikeDrift(frame, 1);

    for (const box of this.boxes) {
      const dx = Number(box.dataset.dx ?? 0);
      const dy = Number(box.dataset.dy ?? 0) + drift * depthDistance(box, 70);
      const rot = Number(box.dataset.rot ?? 0);

      box.style.transform = frame.reduceMotion
        ? `translate(${(dx * 0.55).toFixed(1)}px, ${(dy * 0.55).toFixed(1)}px)`
        : `translate(${(dx * spread).toFixed(1)}px, ${(dy * spread).toFixed(1)}px) rotate(${(rot * spread).toFixed(2)}deg)`;
    }
  }

  private text(frame: ForgeFrame) {
    const spread = frame.text;

    if (!showLayer(this.textLayer, spread, partsOpacity(spread))) {
      return;
    }

    const drift = strikeDrift(frame, 2);

    for (const glyph of this.glyphs) {
      const dx = Number(glyph.dataset.dx ?? 0);
      const dy = Number(glyph.dataset.dy ?? 0) + drift * depthDistance(glyph, 70);
      const scale = 1 + (Number(glyph.dataset.scale ?? 1) - 1) * spread;

      glyph.style.transform = frame.reduceMotion
        ? `translate(${(dx * 0.6).toFixed(1)}px, ${(dy * 0.6).toFixed(1)}px)`
        : `translate(${(dx * spread).toFixed(1)}px, ${(dy * spread).toFixed(1)}px) scale(${scale.toFixed(3)})`;
    }
  }

  private paint(frame: ForgeFrame) {
    const spread = frame.paint;

    if (!showLayer(this.paintLayer, spread, spread > 0.01 ? "1" : "0")) {
      return;
    }

    const drift = strikeDrift(frame, 3);
    const stack = isometricStack(1 - (1 - spread) ** 3, drift);

    this.planes.forEach((plane, index) => {
      const rank = index - (this.planes.length - 1) / 2;
      const lift = -rank * PLANE_GAP * stack.spread + drift * index * 14;

      plane.style.opacity = Math.min(
        1,
        Math.max(0, (spread - index * PLANE_STAGGER) / 0.2),
      ).toFixed(3);
      plane.style.transform = `translate(0px, ${lift.toFixed(1)}px) ${stack.matrix}`;
    });
  }

  /** The caption's drifting lines, and the hollow word receding behind any spread of parts. */
  private backdrop({ progress, step, reduceMotion, layout, text, paint }: ForgeFrame) {
    const captionDrift = reduceMotion ? 0 : stepDrift(progress, step);
    const word = this.words[step];

    this.captionLines[step]?.forEach((line, index) => {
      line.style.transform = `translateY(${(-captionDrift * (index + 1) * 10).toFixed(1)}px)`;
    });
    if (this.backdropLayer) {
      this.backdropLayer.style.opacity = (1 - 0.6 * Math.max(layout, text, paint)).toFixed(3);
    }
    if (word && !reduceMotion) {
      const { stretch, weight } = wordAxes(progress, step);

      word.style.fontStretch = `${stretch.toFixed(1)}%`;
      word.style.fontWeight = weight.toFixed(0);
    }
  }

  /** Marks `step`'s caption, tab, and strike label current, once per change. */
  private syncStep(step: number) {
    if (step === this.activeStep) {
      return;
    }

    this.activeStep = step;
    for (const block of this.copyBlocks) {
      const isCurrent = Number(block.dataset.stageCopy) === step;

      block.toggleAttribute("data-current", isCurrent);
      block.inert = !isCurrent;
    }
    this.stepButtons.forEach((button, index) => {
      if (index === step) {
        button.setAttribute("aria-current", "step");
      } else {
        button.removeAttribute("aria-current");
      }
    });
    if (this.strikeLabel) {
      this.strikeLabel.textContent = STRIKE_LABELS[step] ?? STRIKE_LABELS[0];
    }
  }
}
