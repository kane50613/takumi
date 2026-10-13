"use client";

import { useEffect, useRef } from "react";
import type { PixelRelief } from "./pixel-relief";

/** Sets the stage's state from the step being read, and starts the pixel field near its step. */
export function StoryController() {
  const marker = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    const story = marker.current?.closest("[data-story]");
    const stage = story?.querySelector<HTMLElement>("[data-stage]");
    const steps = story?.querySelectorAll<HTMLElement>("[data-step]");

    if (!story || !stage || !steps) {
      return;
    }

    const image = stage.querySelector<HTMLImageElement>(".story-image");
    const motion = stage.querySelector<HTMLImageElement>(".story-motion");
    const canvas = stage.querySelector<HTMLCanvasElement>(".story-pixels");
    let relief: PixelRelief | undefined;
    let disposed = false;
    let loading = false;

    const loadMotion = () => {
      if (motion && !motion.src && motion.dataset.src) {
        motion.src = motion.dataset.src;
      }
    };
    const loadPixels = () => {
      if (loading || !canvas || !image) {
        return;
      }
      loading = true;
      void Promise.all([import("./pixel-relief"), image.decode()]).then(
        ([{ createPixelRelief }]) => {
          if (disposed) {
            return;
          }
          relief = createPixelRelief(canvas, image);
          if (relief) {
            stage.dataset.pixels = "ready";
            relief.setActive(stage.dataset.state === "pixels");
          }
        },
      );
    };

    const show = (state: string) => {
      stage.dataset.state = state;
      relief?.setActive(state === "pixels");
      if (state === "paint" || state === "pixels") {
        loadPixels();
      }
      if (state === "pixels" || state === "motion") {
        loadMotion();
      }
    };

    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) {
          const state = entry.target instanceof HTMLElement ? entry.target.dataset.step : undefined;

          if (entry.isIntersecting && state) {
            show(state);
          }
        }
      },
      { rootMargin: "-55% 0px -44% 0px" },
    );

    for (const step of steps) {
      observer.observe(step);
    }

    return () => {
      disposed = true;
      observer.disconnect();
      relief?.dispose();
    };
  }, []);

  return <span ref={marker} hidden />;
}
