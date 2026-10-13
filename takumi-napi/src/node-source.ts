import type { Node } from "../index";

const LONE_SURROGATE = /\p{Cs}/u;

/**
 * `node` as JSON, which the native side parses off the main thread, when JSON keeps every value
 * in it; `node` itself otherwise.
 */
export function toNativeSource(node: Node): Node | string {
  try {
    return JSON.stringify(node, function (this: Record<string, unknown>, key, value: unknown) {
      if (this[key] !== value || LONE_SURROGATE.test(key) || !isJsonValue(value)) {
        throw node;
      }

      return value ?? null;
    });
  } catch {
    return node;
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
