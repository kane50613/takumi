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
    const hammer = stage?.querySelector<SVGGElement>("[data-hammer]");
    const anvil = stage?.querySelector<SVGGElement>("[data-anvil]");
    const impactBurst = stage?.querySelector<SVGGElement>("[data-impact-burst]");
    const anvilCard = stage?.querySelector<SVGGElement>("[data-anvil-card]");
    const layoutLayer = stage?.querySelector<SVGGElement>("[data-layer='layout']");
    const textLayer = stage?.querySelector<SVGGElement>("[data-layer='text']");
    const paintLayer = stage?.querySelector<SVGGElement>("[data-layer='paint']");
    const boxElements = [...(stage?.querySelectorAll<SVGGElement>("[data-box]") ?? [])];
    const glyphElements = [...(stage?.querySelectorAll<SVGGElement>("[data-glyph-run]") ?? [])];
    const paintPlanes = [...(stage?.querySelectorAll<SVGGElement>("[data-paint-plane]") ?? [])];
    const copyBlocks = [...(stage?.querySelectorAll<HTMLElement>("[data-stage-copy]") ?? [])];
    const words = [...(stage?.querySelectorAll<HTMLElement>("[data-stage-word]") ?? [])];
    const backdrop = stage?.querySelector<HTMLElement>("[data-stage-backdrop]");
    const stepButtons = [
      ...(stage?.querySelectorAll<HTMLButtonElement>("[data-step-button]") ?? []),
    ];
    const strikeTriggers = [
      ...(stage?.querySelectorAll<HTMLElement>("[data-strike-trigger]") ?? []),
    ];
    const strikeLabel = stage?.querySelector<HTMLElement>("[data-strike-label]");
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
    let activeStep = -1;

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

    const scrollToProgress = (nextProgress: number) => {
      const { rect, pinnedTop, travel } = getTravel();

      if (travel <= 0) {
        return;
      }
      const stageTopInDoc = rect.top + window.scrollY - pinnedTop;
      const destination = stageTopInDoc + nextProgress * travel;

      window.scrollTo({
        top: destination,
        behavior: reduceMotion ? "auto" : "smooth",
      });
    };

    const onStrike = () => {
      const nextStop = STRIKE_STOPS.find((stop) => stop > target + 0.08);

      if (nextStop !== undefined) {
        scrollToProgress(nextStop);
        return;
      }

      const outputsSection = document.getElementById("outputs");

      if (outputsSection) {
        outputsSection.scrollIntoView({ behavior: reduceMotion ? "auto" : "smooth" });
      }
    };

    const apply = (p: number) => {
      const step = Math.max(
        0,
        STEP_STARTS.findLastIndex((start) => p >= start),
      );
      const angle = reduceMotion ? (step === 0 ? -20 : -8) : hammerAngle(p);
      const impulse = reduceMotion ? 0 : strikeImpulse(p);
      const lSpread = reduceMotion ? (step === 1 ? 1 : 0) : layoutSpread(p);
      const tSpread = reduceMotion ? (step === 2 ? 1 : 0) : textSpread(p);
      const pSpread = reduceMotion ? (step === 3 ? 1 : 0) : paintSpread(p);

      if (hammer) {
        hammer.style.transform = `rotate(${-angle.toFixed(2)}deg)`;
      }
      if (anvil) {
        anvil.style.transform = reduceMotion ? "" : `translateY(${(impulse * 12).toFixed(2)}px)`;
      }
      if (impactBurst) {
        impactBurst.style.opacity = impulse.toFixed(3);
        impactBurst.style.transform = `scale(${(0.72 + impulse * 0.62).toFixed(3)})`;
      }

      const intactOpacity = Math.max(0, 1 - Math.max(lSpread, tSpread, pSpread) / CARD_HANDOFF);

      if (anvilCard) {
        anvilCard.style.opacity = intactOpacity.toFixed(3);
        anvilCard.style.transform = reduceMotion
          ? ""
          : `translateY(${(impulse * 9).toFixed(2)}px) scale(${(1 - 0.04 * (1 - intactOpacity)).toFixed(3)})`;
      }

      if (layoutLayer) {
        layoutLayer.style.opacity = partsOpacity(lSpread);
        layoutLayer.style.visibility = lSpread > 0.01 ? "visible" : "hidden";
      }
      const layoutDrift = reduceMotion ? 0 : -stepDrift(p, 1);

      for (const boxEl of lSpread > 0.01 ? boxElements : []) {
        const dx = Number(boxEl.dataset.dx ?? 0);
        const dy = Number(boxEl.dataset.dy ?? 0) + layoutDrift * depthDistance(boxEl, 70);
        const rot = Number(boxEl.dataset.rot ?? 0);

        boxEl.style.transform = reduceMotion
          ? `translate(${(dx * 0.55).toFixed(1)}px, ${(dy * 0.55).toFixed(1)}px)`
          : `translate(${(dx * lSpread).toFixed(1)}px, ${(dy * lSpread).toFixed(1)}px) rotate(${(rot * lSpread).toFixed(2)}deg)`;
      }

      if (textLayer) {
        textLayer.style.opacity = partsOpacity(tSpread);
        textLayer.style.visibility = tSpread > 0.01 ? "visible" : "hidden";
      }
      const textDrift = reduceMotion ? 0 : -stepDrift(p, 2);

      for (const glyphEl of tSpread > 0.01 ? glyphElements : []) {
        const dx = Number(glyphEl.dataset.dx ?? 0);
        const dy = Number(glyphEl.dataset.dy ?? 0) + textDrift * depthDistance(glyphEl, 70);
        const scale = Number(glyphEl.dataset.scale ?? 1);
        const currentScale = 1 + (scale - 1) * tSpread;

        glyphEl.style.transform = reduceMotion
          ? `translate(${(dx * 0.6).toFixed(1)}px, ${(dy * 0.6).toFixed(1)}px)`
          : `translate(${(dx * tSpread).toFixed(1)}px, ${(dy * tSpread).toFixed(1)}px) scale(${currentScale.toFixed(3)})`;
      }

      if (paintLayer) {
        paintLayer.style.opacity = pSpread > 0.01 ? "1" : "0";
        paintLayer.style.visibility = pSpread > 0.01 ? "visible" : "hidden";
      }
      const paintDrift = reduceMotion ? 0 : -stepDrift(p, 3);
      const stack = isometricStack(1 - (1 - pSpread) ** 3, paintDrift);

      (pSpread > 0.01 ? paintPlanes : []).forEach((planeEl, index) => {
        const rank = index - (paintPlanes.length - 1) / 2;
        const lift = -rank * PLANE_GAP * stack.spread + paintDrift * index * 14;

        planeEl.style.opacity = Math.min(
          1,
          Math.max(0, (pSpread - index * PLANE_STAGGER) / 0.2),
        ).toFixed(3);
        planeEl.style.transform = `translate(0px, ${lift.toFixed(1)}px) ${stack.matrix}`;
      });

      const word = words[step];

      if (backdrop) {
        backdrop.style.opacity = (1 - 0.6 * Math.max(lSpread, tSpread, pSpread)).toFixed(3);
      }
      if (word && !reduceMotion) {
        const { stretch, weight } = wordAxes(p, step);

        word.style.fontStretch = `${stretch.toFixed(1)}%`;
        word.style.fontWeight = weight.toFixed(0);
      }

      if (step !== activeStep) {
        activeStep = step;
        copyBlocks.forEach((block) => {
          const isCurrent = Number(block.dataset.stageCopy) === step;

          block.toggleAttribute("data-current", isCurrent);
          block.inert = !isCurrent;
        });
        stepButtons.forEach((button, index) => {
          if (index === step) {
            button.setAttribute("aria-current", "step");
          } else {
            button.removeAttribute("aria-current");
          }
        });
        if (strikeLabel) {
          strikeLabel.textContent = STRIKE_LABELS[step] ?? STRIKE_LABELS[0];
        }
      }
    };

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
      wide.removeEventListener("change", frameForge);
      resizeObserver.disconnect();
    };
  }, []);

  return <span ref={anchorRef} hidden />;
}
