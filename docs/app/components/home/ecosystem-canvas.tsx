"use client";

import { useEffect, useRef } from "react";
import type { EcosystemData, Palette } from "./ecosystem-data";
import type { EcosystemScene } from "./ecosystem-scene";

/** Scroll progress eases toward its target with this time constant. */
const FOLLOW_MS = 140;
const AMBIENT_FRAME_MS = 33;
const START_AFTER_MS = 3500;

function readPalette(): Palette {
  const style = getComputedStyle(document.documentElement);

  return {
    foreground: style.getPropertyValue("--foreground").trim() || "#111111",
    muted: style.getPropertyValue("--muted-foreground").trim() || "#71717a",
    accent: style.getPropertyValue("--primary").trim() || "#e11d48",
  };
}

/**
 * The fixed canvas behind the homepage. It loads three.js once the page is idle, then draws the
 * scene at the reader's scroll position until the story ends.
 */
export function EcosystemCanvas({ data }: { data: EcosystemData }) {
  const host = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const element = host.current;
    const canvas = element?.querySelector("canvas");
    const story = element?.closest<HTMLElement>("[data-ecosystem]");
    const chapters = story?.querySelectorAll<HTMLElement>("[data-chapter]");

    if (!element || !canvas || !story || !chapters?.length) {
      return;
    }

    const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;
    const finePointer = matchMedia("(hover: hover) and (pointer: fine)").matches;
    const pointer = { x: 0, y: 0, targetX: 0, targetY: 0 };
    const end = chapters.length - 1;
    let scene: EcosystemScene | undefined;
    let disposed = false;
    let frame = 0;
    let target = 0;
    let progress = 0;
    let last = performance.now();
    let lastAmbient = 0;

    const measure = () => {
      const first = chapters[0]?.getBoundingClientRect();
      const top = element.getBoundingClientRect().top;
      const exact = first ? Math.min(Math.max((top - first.top) / first.height, 0), end + 0.5) : 0;

      target = reduceMotion ? Math.round(exact) : exact;
    };

    const draw = (now: number) => {
      frame = 0;
      if (!scene || document.hidden) {
        return;
      }

      const dt = now - last;
      const follow = 1 - Math.exp(-dt / FOLLOW_MS);

      last = now;
      progress = reduceMotion ? target : progress + (target - progress) * follow;
      pointer.x += (pointer.targetX - pointer.x) * follow;
      pointer.y += (pointer.targetY - pointer.y) * follow;

      const moving =
        Math.abs(target - progress) > 0.0005 ||
        Math.abs(pointer.targetX - pointer.x) + Math.abs(pointer.targetY - pointer.y) > 0.001;
      const fade = Math.min(Math.max((progress - end - 0.1) / 0.35, 0), 1);

      element.style.opacity = String(1 - fade);
      element.style.visibility = fade >= 1 ? "hidden" : "visible";

      const ambient =
        fade < 1 &&
        (moving || now - lastAmbient >= AMBIENT_FRAME_MS) &&
        scene.render(progress, now, pointer);

      if (ambient) {
        lastAmbient = now;
      }
      if (moving || ambient) {
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

    const onPointer = (event: PointerEvent) => {
      pointer.targetX = (event.clientX / innerWidth) * 2 - 1;
      pointer.targetY = (event.clientY / innerHeight) * 2 - 1;
      wake();
    };
    const resizeObserver = new ResizeObserver(() => {
      scene?.resize(canvas.clientWidth, canvas.clientHeight);
      wake();
    });
    const themeObserver = new MutationObserver(() => {
      scene?.setPalette(readPalette());
      wake();
    });

    let started = false;
    const start = () => {
      if (started) {
        return;
      }
      started = true;

      const svgs = [...story.querySelectorAll<SVGSVGElement>("[data-layer-svg] > svg")];

      void import("./ecosystem-scene")
        .then(({ createEcosystemScene }) =>
          createEcosystemScene(canvas, data, svgs, readPalette(), reduceMotion),
        )
        .then((created) => {
          if (disposed) {
            created.dispose();
            return;
          }
          scene = created;
          story.dataset.scene = "ready";
          wake();
        });
    };
    // The poster already shows the hero, so three.js waits for the reader to act or for a quiet
    // moment well after load, and stays off the page's critical path.
    const triggers = ["scroll", "pointerdown", "keydown", "touchstart"] as const;
    const startSoon = setTimeout(() => {
      if ("requestIdleCallback" in window) {
        requestIdleCallback(start, { timeout: 2000 });
      } else {
        start();
      }
    }, START_AFTER_MS);

    for (const type of triggers) {
      addEventListener(type, start, { once: true, passive: true });
    }

    const cancelStart = () => {
      clearTimeout(startSoon);
      for (const type of triggers) {
        removeEventListener(type, start);
      }
    };

    measure();
    addEventListener("scroll", wake, { passive: true });
    document.addEventListener("visibilitychange", wake);
    if (finePointer && !reduceMotion) {
      addEventListener("pointermove", onPointer, { passive: true });
    }
    resizeObserver.observe(canvas);
    themeObserver.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class"],
    });

    return () => {
      disposed = true;
      cancelStart();
      cancelAnimationFrame(frame);
      removeEventListener("scroll", wake);
      document.removeEventListener("visibilitychange", wake);
      removeEventListener("pointermove", onPointer);
      resizeObserver.disconnect();
      themeObserver.disconnect();
      scene?.dispose();
    };
  }, [data]);

  return (
    <div
      ref={host}
      aria-hidden
      className="ecosystem-canvas pointer-events-none fixed inset-x-0 top-14 bottom-0 z-0"
    >
      <canvas className="size-full opacity-0 transition-opacity duration-500 ease-out" />
    </div>
  );
}
