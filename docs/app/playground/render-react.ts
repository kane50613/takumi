import * as React from "react";
import { createElement, type ElementType, type ReactNode } from "react";

// Mirror Takumi's `tw` into `className` (keeping `tw`) so one evaluated tree
// serves both the Takumi render (reads `tw`) and the browser preview (reads `class`).
function mirrorTw<P>(props: P): P {
  if (!props || typeof props !== "object" || !("tw" in props)) return props;
  const { tw, className, class: klass } = props as Record<string, unknown>;
  return { ...props, className: [className ?? klass, tw].filter(Boolean).join(" ") };
}

/** The React a template renders with. */
export const renderReact: typeof React = {
  ...React,
  createElement: ((
    type: ElementType,
    props: Record<string, unknown> | null,
    ...children: ReactNode[]
  ) => createElement(type, mirrorTw(props), ...children)) as typeof createElement,
};
