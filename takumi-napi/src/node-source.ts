import type { HtmlSource, Node } from "../index";

const LONE_SURROGATE = /\p{Cs}/u;

/** The most arrays and objects `serde_json` reads nested inside each other. */
const MAX_DEPTH = 127;

/**
 * A node tree as JSON, which the native side parses off the main thread, when JSON keeps every
 * value in it within {@link MAX_DEPTH}; the tree itself otherwise, and an `HtmlSource` as is.
 */
export function toNativeSource(source: Node | HtmlSource): Node | HtmlSource | string {
  if ("html" in source) {
    return source;
  }

  const depths = new WeakMap<object, number>();

  try {
    return JSON.stringify(source, function (this: Record<string, unknown>, key, value: unknown) {
      if (this[key] !== value || LONE_SURROGATE.test(key) || !isJsonValue(value)) {
        throw source;
      }

      if (typeof value === "object" && value !== null) {
        const depth = (depths.get(this) ?? 0) + 1;

        if (depth > MAX_DEPTH) {
          throw source;
        }

        depths.set(value, depth);
      }

      return value ?? null;
    });
  } catch {
    return source;
  }
}

/** Whether JSON keeps `value` as the native side reads it from an object. */
function isJsonValue(value: unknown): boolean {
  switch (typeof value) {
    case "string":
      return !LONE_SURROGATE.test(value);
    case "number":
      return Number.isFinite(value);
    case "boolean":
    case "undefined":
      return true;
    case "object":
      return (
        value === null ||
        Array.isArray(value) ||
        [Object.prototype, null].includes(Object.getPrototypeOf(value))
      );
    default:
      return false;
  }
}
