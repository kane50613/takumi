import type * as React from "react";
import { transform } from "sucrase";
import * as primitives from "takumi-pdf/primitives";
import * as z from "zod/mini";
import { optionsSchema } from "./schema";

/// Modules a template may import. The wasm entry stays out: the playground
/// renders through its own worker.
const MODULES: Record<string, unknown> = {
  "takumi-pdf/primitives": primitives,
};

// zrender reads the Node `global`, which a worker does not define.
function shimGlobal() {
  if (!("global" in globalThis)) Reflect.set(globalThis, "global", globalThis);
}

/// Heavy modules fetched as their own chunk the first time a template imports them.
const LAZY_MODULES: Record<string, () => Promise<unknown>> = {
  "echarts/core": () => import("echarts/core"),
  "echarts/charts": () => import("echarts/charts"),
  "echarts/components": () => import("echarts/components"),
  "echarts/renderers": () => import("echarts/renderers"),
};

async function loadLazyModules(transformed: string) {
  const ids = new Set(
    [...transformed.matchAll(/\brequire\((['"])([^'"]+)\1\)/g)].map((match) => match[2]),
  );

  const pending = Object.entries(LAZY_MODULES).filter(([id]) => ids.has(id) && !(id in MODULES));

  // Every lazy module is echarts today.
  if (pending.length > 0) shimGlobal();

  await Promise.all(
    pending.map(async ([id, load]) => {
      MODULES[id] = await load();
    }),
  );
}

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

export async function evaluateCodeExports(code: string, react: typeof React) {
  const transformed = transformCode(code);

  await loadLazyModules(transformed);

  const exports = {};

  new Function("exports", "require", "React", transformed)(exports, requireModule, react);

  return exportsSchema.parse(exports);
}
