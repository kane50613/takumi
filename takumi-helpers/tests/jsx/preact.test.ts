import { describe, expect, test } from "bun:test";
import { h } from "preact";
import { useMemo, useState } from "preact/hooks";
import { fromJsx } from "../../src/jsx";

describe("preact trees render through preact-render-to-string", () => {
  test("resolves preact hooks", async () => {
    const Counter = () => {
      const [count] = useState(7);
      const label = useMemo(() => `count ${count}`, [count]);

      return h("p", { tw: "font-bold" }, label);
    };

    const { node } = await fromJsx(h("div", null, h(Counter, null)));

    expect(node).toMatchObject({
      type: "container",
      tagName: "div",
      children: [
        {
          type: "text",
          text: "count 7",
          tw: "font-bold",
        },
      ],
    });
  });

  test("keeps inline styles from preact host elements", async () => {
    const { node } = await fromJsx(h("div", { style: { backgroundColor: "red" } }, "styled"));

    expect(node).toMatchObject({
      type: "text",
      text: "styled",
      style: expect.objectContaining({ backgroundColor: "red" }),
    });
  });
});
