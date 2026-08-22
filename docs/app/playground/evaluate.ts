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
