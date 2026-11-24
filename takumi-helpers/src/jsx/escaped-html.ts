/** The markup Hono's `html` template and its components such as `Suspense` return. */
export type EscapedHtml = {
  toString(): string;
  isEscaped: true;
  callbacks?: EscapedHtmlCallback[];
};

type EscapedHtmlCallback = (options: {
  phase: number;
  buffer: [string];
  context: object;
}) => unknown;

/** Hono's `HtmlEscapedCallbackPhase.Stringify`, the phase `c.html()` resolves markup in. */
const STRINGIFY = 1;

export function isEscapedHtml(value: unknown): value is EscapedHtml {
  return value instanceof String && (value as { isEscaped?: unknown }).isEscaped === true;
}

/**
 * The markup `html` stands for once its callbacks ran, as `c.html()` resolves it, so a `Suspense`
 * boundary holds its children rather than its fallback.
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
