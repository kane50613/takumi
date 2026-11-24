import type { HtmlEscaped, HtmlEscapedCallbackPhase } from "hono/utils/html";

/** A Hono JSX element, or the markup Hono's `html` template and components such as `Suspense` return. */
type EscapedHtml = HtmlEscaped & { toString(): string | Promise<string> };

const STRINGIFY: (typeof HtmlEscapedCallbackPhase)["Stringify"] = 1;

export function isEscapedHtml(value: unknown): value is EscapedHtml {
  return (
    typeof value === "object" && value !== null && "isEscaped" in value && value.isEscaped === true
  );
}

/**
 * The markup `c.html()` sends for `html`, rendered and resolved by Hono, so context, `Suspense`,
 * and `ErrorBoundary` behave as Hono defines them.
 * <https://github.com/honojs/hono/blob/main/src/utils/html.ts>
 */
export function resolveEscapedHtml(html: EscapedHtml): Promise<string> {
  return resolve(html, {});
}

async function resolve(value: unknown, context: object, buffer?: [string]): Promise<string> {
  const html: unknown = await (typeof value === "object" &&
  value !== null &&
  !(value instanceof String) &&
  !(value instanceof Promise)
    ? value.toString()
    : value);
  const callbacks = isEscapedHtml(html) ? html.callbacks : undefined;

  if (!callbacks?.length) {
    return String(html);
  }

  if (buffer) {
    buffer[0] += String(html);
  } else {
    buffer = [String(html)];
  }

  const results = await Promise.all(
    callbacks.map((callback) => callback({ phase: STRINGIFY, buffer, context })),
  );

  await Promise.all(results.filter(Boolean).map((result) => resolve(result, context, buffer)));

  return buffer[0];
}
