import * as z from "zod/mini";
import type { PdfInspection } from "./inspect-pdf";
import type { PlaygroundPdfOptions } from "./options";

const declarationsSchema = z.record(z.string(), z.union([z.string(), z.number()]));

/**
 * A `css` entry. Declared here rather than taken from `takumi-js`, whose type
 * reaches this app through a re-export that resolves to `any`.
 */
export type CssEntry =
  | string
  | StyleRuleEntry
  | { keyframes: string; steps: { offset: string; style?: Declarations }[] }
  | { media: string; rules: CssEntry[] }
  | { supports: string; rules: CssEntry[] }
  | { layer: string; rules?: CssEntry[] };

/** A style rule nests style rules alone, as CSS nesting. */
type StyleRuleEntry = { selector: string; style?: Declarations; rules?: StyleRuleEntry[] };

type Declarations = Record<string, string | number>;

// `strictObject`, so a typo like `declarations` is an error rather than a rule
// the renderer never sees.
const styleRuleSchema: z.ZodMiniType<StyleRuleEntry> = z.lazy(() =>
  z.strictObject({
    selector: z.string(),
    style: z.optional(declarationsSchema),
    rules: z.optional(z.array(styleRuleSchema)),
  }),
);

const cssInputSchema: z.ZodMiniType<CssEntry> = z.lazy(() =>
  z.union([
    z.string(),
    z.strictObject({
      keyframes: z.string(),
      steps: z.array(z.strictObject({ offset: z.string(), style: z.optional(declarationsSchema) })),
    }),
    z.strictObject({ media: z.string(), rules: z.array(cssInputSchema) }),
    z.strictObject({ supports: z.string(), rules: z.array(cssInputSchema) }),
    z.strictObject({ layer: z.string(), rules: z.optional(z.array(cssInputSchema)) }),
    styleRuleSchema,
  ]),
);

export const optionsSchema = z.object({
  width: z.optional(z.int().check(z.positive(), z.minimum(1))),
  height: z.optional(z.int().check(z.positive(), z.minimum(1))),
  quality: z.optional(z.int().check(z.positive(), z.minimum(1), z.maximum(100))),
  format: z.optional(z.enum(["png", "jpeg", "webp"])),
  devicePixelRatio: z.optional(z.number().check(z.positive(), z.minimum(0.1), z.maximum(10.0))),
  css: z.optional(z.union([cssInputSchema, z.array(cssInputSchema)])),
  animation: z.optional(
    z.object({
      durationMs: z.int().check(z.positive(), z.minimum(1)),
      fps: z.optional(z.int().check(z.positive(), z.minimum(1))),
      format: z.optional(z.enum(["webp", "apng", "gif"])),
    }),
  ),
  // The renderer validates these; the playground only forwards them. They may hold
  // JSX (`header`, `footer`), so they never travel back to the main thread.
  pdf: z.optional(z.custom<PlaygroundPdfOptions>()),
  emoji: z.optional(z.enum(["twemoji", "blobmoji", "noto", "openmoji"])),
});

export const outputKinds = ["image", "animation", "pdf"] as const;

export type OutputKind = (typeof outputKinds)[number];

const renderSuccessSchema = z.object({
  status: z.literal("success"),
  id: z.int().check(z.positive(), z.minimum(1)),
  outputBuffer: z.any(),
  outputUrl: z.optional(z.string()),
  duration: z.number(),
  outputKind: z.enum(outputKinds),
  outputFormat: z.string(),
  /** Human-readable geometry for the status bar, e.g. `1200 × 630` or `A4`. */
  label: z.string(),
  /** What the PDF bytes turned out to contain, for PDF output. */
  inspection: z.optional(z.custom<PdfInspection>()),
  /** What degraded in this render, shown in the status bar. */
  notice: z.optional(z.string()),
});

const renderErrorSchema = z.object({
  status: z.literal("error"),
  id: z.int().check(z.positive(), z.minimum(1)),
  message: z.string(),
  transformedCode: z.optional(z.string()),
});

const renderRequestSchema = z.object({
  type: z.literal("render-request"),
  id: z.int().check(z.positive(), z.minimum(1)),
  code: z.string(),
});

export const renderResultSchema = z.object({
  type: z.literal("render-result"),
  result: z.discriminatedUnion("status", [renderSuccessSchema, renderErrorSchema]),
});

const readySchema = z.object({
  type: z.literal("ready"),
});

// Carries the port the watchdog pings over. The port stays out of the worker
// global, where the evaluated code could answer for it.
const watchdogSchema = z.object({
  type: z.literal("watchdog"),
});

// Posted before the (slower) fetches + WASM render so the browser pane never
// waits on them. `cssContents` is raw CSS (Takumi's effective stylesheets), not URLs.
const previewResultSchema = z.object({
  type: z.literal("preview-result"),
  id: z.int().check(z.positive(), z.minimum(1)),
  html: z.string(),
  width: z.optional(z.int().check(z.positive(), z.minimum(1))),
  // Omitted for paged PDF: the pane shows one continuous flow instead of pages.
  height: z.optional(z.int().check(z.positive(), z.minimum(1))),
  /** CSS `padding` shorthand mirroring the PDF page margin. */
  padding: z.optional(z.string()),
  cssContents: z.optional(z.array(z.string())),
  /** `:root` declarations the pane's own Tailwind compiler needs. */
  theme: z.optional(z.string()),
});

export const messageSchema = z.discriminatedUnion("type", [
  renderRequestSchema,
  renderResultSchema,
  readySchema,
  watchdogSchema,
  previewResultSchema,
]);

export type RenderMessageInput = z.input<typeof messageSchema>;
