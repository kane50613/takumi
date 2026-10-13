"use client";

import { useEffect, useRef } from "react";
import type { Palette, ParticleScene, StageData, StageLayout } from "./particle-scene";
import { ramp, STAGE, STEP_STARTS } from "./stage-timeline";

/** Scroll progress eases toward its target with this time constant. */
const FOLLOW_MS = 120;
const START_AFTER_MS = 3500;
const WIDE_MIN = 1024;

function readPalette(): Palette {
  const style = getComputedStyle(document.documentElement);

  return {
    foreground: style.getPropertyValue("--foreground").trim() || "#111111",
    background: style.getPropertyValue("--background").trim() || "#fafafa",
  };
}

function canRunScene() {
  const connection = (navigator as Navigator & { connection?: { saveData?: boolean } }).connection;

  return (
    !matchMedia("(prefers-reduced-motion: reduce)").matches &&
    connection?.saveData !== true &&
    document.createElement("canvas").getContext("webgl2") !== null
  );
}

/**
 * Drives the homepage stage from scroll: the hero leaves, the headline hands over to the particle
 * scene, and the steps track the phase. Without the scene, the headline fades and the card fades in.
 */
export function StageController({ data }: { data: StageData }) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    const stage = canvas?.closest<HTMLElement>("[data-stage]");
    const view = stage?.querySelector<HTMLElement>("[data-stage-view]");
    const hero = stage?.querySelector<HTMLElement>("[data-hero]");
    const headline = stage?.querySelector<HTMLElement>("[data-headline]");
    const card = stage?.querySelector<HTMLElement>("[data-card]");
    const steps = stage?.querySelector<HTMLElement>("[data-steps]");
    const stepItems = [...(steps?.querySelectorAll<HTMLElement>("li") ?? [])];

    if (!canvas || !stage || !view || !hero || !headline || !card || !steps) {
      return;
    }

    const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;
    let scene: ParticleScene | undefined;
    let sampleInk: ((heading: HTMLElement, origin: DOMRect) => Float32Array) | undefined;
    let disposed = false;
    let frame = 0;
    let target = 0;
    let progress = 0;
    let last = performance.now();
    let activeStep = -1;

    const measure = () => {
      const rect = stage.getBoundingClientRect();
      const pinnedTop = parseFloat(getComputedStyle(view).top) || 0;
      const travel = rect.height - view.offsetHeight;

      target = travel > 0 ? Math.min(Math.max((pinnedTop - rect.top) / travel, 0), 1) : 0;
    };

    const stageLayout = (): StageLayout => {
      const origin = view.getBoundingClientRect();
      const cardRect = card.getBoundingClientRect();

      return {
        width: origin.width,
        height: origin.height,
        ink: sampleInk?.(headline, origin) ?? new Float32Array(),
        card: {
          left: cardRect.left - origin.left,
          top: cardRect.top - origin.top,
          width: cardRect.width,
          height: cardRect.height,
        },
      };
    };

    const apply = (p: number) => {
      const heroOut = ramp(p, STAGE.heroOut);
      const step = STEP_STARTS.findLastIndex((start) => p >= start);

      hero.style.opacity = String(1 - heroOut);
      hero.style.transform = reduceMotion ? "" : `translateY(${-heroOut * 12}svh)`;
      hero.style.visibility = heroOut >= 1 ? "hidden" : "";
      hero.inert = heroOut > 0.5;
      steps.style.opacity = String(ramp(p, STAGE.steps));

      if (scene) {
        const handedOff = p >= STAGE.handoff;

        headline.style.opacity = handedOff ? "0" : "1";
        card.style.opacity = String(ramp(p, STAGE.image));
        canvas.style.visibility = handedOff && p < STAGE.image[1] ? "visible" : "hidden";
        if (canvas.style.visibility === "visible") {
          scene.render(p);
        }
      } else {
        headline.style.opacity = String(1 - heroOut);
        card.style.opacity = String(ramp(p, [0.2, 0.4]));
      }

      if (step !== activeStep) {
        activeStep = step;
        stepItems.forEach((item, index) => {
          if (index === step) {
            item.setAttribute("aria-current", "step");
          } else {
            item.removeAttribute("aria-current");
          }
        });
      }
    };

    const draw = (now: number) => {
      frame = 0;

      const follow = reduceMotion ? 1 : 1 - Math.exp(-(now - last) / FOLLOW_MS);

      last = now;
      progress += (target - progress) * follow;
      if (Math.abs(target - progress) < 0.0004) {
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

    const relayout = () => {
      if (scene) {
        scene.layout(stageLayout());
        apply(progress);
      }
      wake();
    };

    let started = false;
    const start = async () => {
      if (started || !canRunScene()) {
        return;
      }
      started = true;

      await document.fonts.ready;

      const module = await import("./particle-scene");
      const created = await module.createParticleScene(canvas, data, {
        columns: view.clientWidth >= WIDE_MIN ? 200 : 110,
        palette: readPalette(),
      });

      if (disposed) {
        created.dispose();
        return;
      }
      scene = created;
      sampleInk = module.sampleInk;
      scene.layout(stageLayout());
      apply(progress);
    };

    // The page reads complete without the scene, so three.js waits for the reader to act or for
    // a quiet moment well after load.
    const triggers = ["scroll", "pointerdown", "keydown", "touchstart"] as const;
    const onTrigger = () => void start();
    const startSoon = setTimeout(() => {
      if ("requestIdleCallback" in window) {
        requestIdleCallback(onTrigger, { timeout: 2000 });
      } else {
        onTrigger();
      }
    }, START_AFTER_MS);

    for (const type of triggers) {
      addEventListener(type, onTrigger, { once: true, passive: true });
    }

    const resizeObserver = new ResizeObserver(relayout);
    const themeObserver = new MutationObserver(() => {
      scene?.setPalette(readPalette());
      apply(progress);
    });

    measure();
    progress = target;
    apply(progress);
    addEventListener("scroll", wake, { passive: true });
    resizeObserver.observe(view);
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class"],
    });

    return () => {
      disposed = true;
      clearTimeout(startSoon);
      for (const type of triggers) {
        removeEventListener(type, onTrigger);
      }
      cancelAnimationFrame(frame);
      removeEventListener("scroll", wake);
      resizeObserver.disconnect();
      themeObserver.disconnect();
      scene?.dispose();
    };
  }, [data]);

  return (
    <canvas
      ref={canvasRef}
      aria-hidden
      className="pointer-events-none absolute inset-0 size-full"
      style={{ visibility: "hidden" }}
    />
  );
}
