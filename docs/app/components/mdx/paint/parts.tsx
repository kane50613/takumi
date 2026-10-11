"use client";

import { type ReactNode, type RefObject, useEffect, useRef, useState } from "react";
import { cn } from "~/lib/utils";
import { round } from "~/playground/inspect-paint";
import type { PaintScene, SceneStep } from "./scene";

/** Whether `element` is on screen and the reader has not asked for less motion. */
export function useAutoplayAllowed(element: RefObject<Element | null>) {
  const [allowed, setAllowed] = useState(false);

  useEffect(() => {
    const target = element.current;
    const reduced = matchMedia("(prefers-reduced-motion: reduce)");

    if (!target) return;

    let visible = false;
    const update = () => setAllowed(visible && !reduced.matches);
    const observer = new IntersectionObserver(
      ([entry]) => {
        visible = entry?.isIntersecting ?? false;
        update();
      },
      { rootMargin: "-20% 0px" },
    );

    observer.observe(target);
    reduced.addEventListener("change", update);
    return () => {
      observer.disconnect();
      reduced.removeEventListener("change", update);
    };
  }, [element]);

  return allowed;
}

/**
 * How far, from `0` to `1`, a timer of `duration` milliseconds has run while `running`. It
 * starts over when `key` changes and calls `onDone` once it reaches `1`.
 */
export function useProgress({
  duration,
  running,
  key,
  onDone,
}: {
  duration: number;
  running: boolean;
  key: unknown;
  onDone: () => void;
}) {
  const [progress, setProgress] = useState(0);
  const elapsed = useRef(0);
  const done = useRef(onDone);

  useEffect(() => {
    done.current = onDone;
  });

  useEffect(() => {
    elapsed.current = 0;
    setProgress(0);
  }, [key]);

  useEffect(() => {
    if (!running) return;

    let last: number | undefined;
    let frame = requestAnimationFrame(function tick(now) {
      // A hidden tab pauses frames; the gap it leaves is not playback time.
      elapsed.current += Math.min(now - (last ?? now), 100);
      last = now;

      const next = Math.min(elapsed.current / duration, 1);

      setProgress(next);
      if (next >= 1) return done.current();
      frame = requestAnimationFrame(tick);
    });

    return () => cancelAnimationFrame(frame);
  }, [running, duration, key]);

  return progress;
}

export function Surface({
  children,
  ref,
}: {
  children: ReactNode;
  ref?: RefObject<HTMLElement | null>;
}) {
  return (
    <figure
      ref={ref}
      className="not-prose my-6 overflow-hidden rounded-2xl border bg-fd-card text-sm shadow-sm"
    >
      {children}
    </figure>
  );
}

export function Toolbar({ children }: { children: ReactNode }) {
  return <div className="flex flex-wrap items-center gap-1.5 border-b px-3 py-2.5">{children}</div>;
}

/** The dotted backdrop a canvas sits on. */
export function Stage({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div
      className={cn(
        "flex items-center justify-center bg-fd-background bg-[radial-gradient(var(--color-fd-border)_1px,transparent_1px)] bg-size-[16px_16px] p-6",
        className,
      )}
    >
      {children}
    </div>
  );
}

export function Canvas({
  scene,
  label,
  children,
}: {
  scene: Pick<PaintScene, "width" | "height">;
  label: string;
  children: ReactNode;
}) {
  return (
    <svg
      role="img"
      aria-label={label}
      viewBox={`0 0 ${scene.width} ${scene.height}`}
      style={{ maxWidth: scene.width * 1.4 }}
      className="h-auto w-full rounded-lg bg-white shadow-[0_1px_2px_rgba(0,0,0,0.06),0_12px_32px_-12px_rgba(0,0,0,0.25)] ring-1 ring-black/5"
    >
      {children}
    </svg>
  );
}

/** A side panel beside the stage; it stacks under it on narrow screens. */
export function Panel({ children, className }: { children: ReactNode; className?: string }) {
  return <div className={cn("border-t p-4 md:border-t-0 md:border-l", className)}>{children}</div>;
}

export function Footer({ children }: { children: ReactNode }) {
  return <div className="flex items-center gap-4 border-t px-4 py-3">{children}</div>;
}

export function Choice({
  pressed,
  onClick,
  children,
}: {
  pressed: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-pressed={pressed}
      onClick={onClick}
      className={cn(
        "rounded-full border px-3 py-1 font-mono text-xs transition-[background-color,border-color,color,transform] duration-150 ease-out active:scale-[0.97]",
        pressed
          ? "border-fd-primary/40 bg-fd-primary/10 text-fd-primary"
          : "border-transparent text-fd-muted-foreground hover:text-fd-foreground",
      )}
    >
      {children}
    </button>
  );
}

export function Readout({ rows }: { rows: [string, ReactNode][] }) {
  return (
    <dl className="grid grid-cols-[auto_minmax(0,1fr)] content-start gap-x-4 gap-y-1.5 font-mono text-xs">
      {rows.map(([label, value]) => (
        <div key={label} className="contents">
          <dt className="text-fd-muted-foreground">{label}</dt>
          <dd className="break-words tabular-nums">{value}</dd>
        </div>
      ))}
    </dl>
  );
}

/** One line naming what `step` does, for a step list. */
export function stepDetail(scene: PaintScene, step: SceneStep) {
  const name = scene.nodes[step.node]?.name ?? "node";

  switch (step.type) {
    case "draw": {
      const roles = new Set(
        step.drawables.map((drawable) => ("role" in drawable ? drawable.role : "group")),
      );

      return `${name} · ${[...roles].join(", ")}`;
    }
    case "begin-group":
      return `${name} · opacity ${round(step.opacity)}`;
    case "begin-clip":
      return `${name} · overflow`;
    case "end-group":
    case "end-clip":
      return name;
  }
}
