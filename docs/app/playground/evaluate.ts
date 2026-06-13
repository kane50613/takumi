import * as React from "react";
import { renderToStaticMarkup } from "react-dom/server";
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

// Remap Takumi's `tw` prop to `class` so the browser preview can style it.
function remapTw<P>(props: P): P {
  if (!props || typeof props !== "object" || !("tw" in props)) return props;
  const { tw, className, class: klass, ...rest } = props as Record<string, unknown>;
  return { ...rest, className: [className ?? klass, tw].filter(Boolean).join(" ") } as P;
}

const browserReact: typeof React = {
  ...React,
  createElement: ((
    type: React.ElementType,
    props: Record<string, unknown> | null,
    ...children: React.ReactNode[]
  ) => React.createElement(type, remapTw(props), ...children)) as typeof React.createElement,
};

export function renderBrowserHtml(code: string) {
  const { default: Component } = evaluateCodeExports(code, browserReact);
  return renderToStaticMarkup(browserReact.createElement(Component as React.FC));
}
