import { z } from "zod/mini";

export const optionsSchema = z.object({
  width: z.int().check(z.positive(), z.minimum(1)),
  height: z.int().check(z.positive(), z.minimum(1)),
  quality: z.optional(
    z.int().check(z.positive(), z.minimum(1), z.maximum(100)),
  ),
  format: z.enum(["png", "jpeg", "webp"]),
});

const renderSuccessSchema = z.object({
  status: z.literal("success"),
  id: z.int().check(z.positive(), z.minimum(1)),
  dataUrl: z.string(),
  duration: z.number(),
  node: z.unknown(),
  options: optionsSchema,
});

const renderErrorSchema = z.object({
  status: z.literal("error"),
  id: z.int().check(z.positive(), z.minimum(1)),
  message: z.string(),
  transformedCode: z.optional(z.string()),
});

export const renderRequestSchema = z.object({
  type: z.literal("render-request"),
  id: z.int().check(z.positive(), z.minimum(1)),
  code: z.string(),
});

export const renderResultSchema = z.object({
  type: z.literal("render-result"),
  result: z.discriminatedUnion("status", [
    renderSuccessSchema,
    renderErrorSchema,
  ]),
});

export const readySchema = z.object({
  type: z.literal("ready"),
});

export const messageSchema = z.discriminatedUnion("type", [
  renderRequestSchema,
  renderResultSchema,
  readySchema,
]);

export type RenderMessageInput = z.input<typeof messageSchema>;
