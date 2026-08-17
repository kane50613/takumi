import * as React from "react";
import { transform } from "sucrase";
import * as primitives from "takumi-pdf/primitives";
import * as z from "zod/mini";
import { optionsSchema } from "./schema";

/// Modules a template may import. The wasm entry stays out: the playground
/// renders through its own worker.
const MODULES: Record<string, unknown> = {
  "takumi-pdf/primitives": primitives,
};

function requireModule(id: string): unknown {
  const module = MODULES[id];

  if (module === undefined) {
    throw new Error(`the playground cannot import "${id}"`);
  }
  return module;
}

const exportsSchema = z.object({
  default: z.function(),
  options: optionsSchema,
});

function transformCode(code: string) {
  return transform(code, {
    transforms: ["jsx", "typescript", "imports"],
    production: true,
  }).code;
}

export function evaluateCodeExports(code: string, react: typeof React) {
  const exports = {};

  new Function("exports", "require", "React", transformCode(code))(exports, requireModule, react);

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
