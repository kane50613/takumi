import { describe, expect, test } from "bun:test";
import { h } from "preact";
import { fromJsx } from "../../src/jsx";

describe("preact trees traverse natively", () => {
  test("renders components", async () => {
    const Label = ({ text }: { text: string }) => h("p", { tw: "font-bold" }, text);

    const { node } = await fromJsx(h("div", null, h(Label, { text: "count 7" })));

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
