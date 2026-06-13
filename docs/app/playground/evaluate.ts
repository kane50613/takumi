import * as React from "react";
import { transform } from "sucrase";
import * as z from "zod/mini";
import { optionsSchema } from "./schema";

const exportsSchema = z.object({
  default: z.function(),
  options: optionsSchema,
});

export function transformCode(code: string) {
  return transform(code, {
    transforms: ["jsx", "typescript", "imports"],
    production: true,
  }).code;
}

export function evaluateCodeExports(code: string, react: typeof React) {
  const exports = {};

  new Function("exports", "React", transformCode(code))(exports, react);

  return exportsSchema.parse(exports);
}

// Mirror Takumi's `tw` into `className` (keeping `tw`) so one evaluated tree
// serves both the Takumi render (reads `tw`) and the browser preview (reads `class`).
function mirrorTw<P>(props: P): P {
  if (!props || typeof props !== "object" || !("tw" in props)) return props;
  const { tw, className, class: klass } = props as Record<string, unknown>;
  return { ...props, className: [className ?? klass, tw].filter(Boolean).join(" ") };
}

export const renderReact: typeof React = {
  ...React,
  createElement: ((
    type: React.ElementType,
    props: Record<string, unknown> | null,
    ...children: React.ReactNode[]
  ) => React.createElement(type, mirrorTw(props), ...children)) as typeof React.createElement,
};
