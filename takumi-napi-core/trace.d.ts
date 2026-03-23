/**
 * @description An array of node_modules path strings that should be included in Turbopack or Nitro's trace for NAPI modules.
 *
 * @example
 * ```ts
 * // next.config.ts
 * import { traceIncludes } from "@takumi-rs/core/trace";
 *
 * export default {
 *   outputFileTracingIncludes: {
 *     "/*": traceIncludes,
 *   },
 * };
 * ```
 */
export const traceIncludes: string[];
