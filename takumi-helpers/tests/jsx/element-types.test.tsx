import { describe, expect, test } from "bun:test";
import { h } from "preact";
import { createElement as createCompatElement } from "preact/compat";
import { createElement, type ReactElement } from "react";
import { fromJsx } from "../../src/jsx";
import type { ReactElementLike } from "../../src/types";

function Component() {
  return <div>text</div>;
}

// Type guards: every element shape a supported runtime hands us stays assignable to
// ReactElementLike, so callers never need a cast. preact/compat types `$$typeof` as
// `symbol | string`, and a propless component widens to `FunctionComponentElement<never>`.
const guards = {
  jsx: (<Component />) satisfies ReactElementLike,
  reactElement: createElement(
    "div",
    null,
    "text",
  ) satisfies ReactElement satisfies ReactElementLike,
  propless: createElement(Component) satisfies ReactElementLike,
  preactVNode: h("div", null, "text") satisfies ReactElementLike,
  preactCompat: createCompatElement("div", null, "text") satisfies ReactElementLike,
};

describe("element types", () => {
  test("every runtime's element renders", async () => {
    for (const element of Object.values(guards)) {
      const { node } = await fromJsx(element);

      expect(node).toMatchObject({ type: "text", text: "text" });
    }
  });
});
