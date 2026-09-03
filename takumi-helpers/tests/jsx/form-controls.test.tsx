import { describe, expect, test } from "bun:test";
import { fromHtml } from "../../src/html";
import { fromJsx } from "../../src/jsx";
import { defaultStylePresets } from "../../src/jsx/style-presets";
import type { Node, TextNode } from "../../src/types";

function children(node: Node): Node[] {
  if (node.type !== "container") {
    throw new Error("expected a container");
  }

  return node.children ?? [];
}

function textOf(node: Node | undefined): string {
  if (node?.type !== "text") {
    throw new Error("expected a text node");
  }

  return node.text;
}

describe("form controls", () => {
  test("an input preset follows its type", async () => {
    const { node: check } = await fromJsx(<input type="Checkbox" />);
    const { node: plain } = await fromJsx(<input />);

    expect(check.preset).toBe(defaultStylePresets["input[type=checkbox]"]);
    expect(plain.preset).toBe(defaultStylePresets.input);
  });

  test("React attribute names reach the node as HTML ones", async () => {
    const { node } = await fromJsx(
      <label htmlFor="who">
        <input id="who" defaultValue="Kane" defaultChecked />
      </label>,
    );
    const input = children(node)[0];

    expect(node.attributes).toEqual({ for: "who" });
    expect(input?.attributes).toEqual({ value: "Kane", checked: "" });
  });

  test("a closed select shows the option it starts on", async () => {
    const { node } = await fromJsx(
      <select name="plan">
        <option value="M">Monthly</option>
        <option value="A" selected>
          Annual
        </option>
      </select>,
    );
    const [shown, ...options] = children(node);

    expect(shown).toEqual({
      type: "text",
      text: "Annual",
      preset: defaultStylePresets.span,
    } satisfies TextNode);
    expect(options.map((option) => option.preset)).toEqual([
      { display: "none" },
      { display: "none" },
    ]);
  });

  test("a select's defaultValue picks its option", async () => {
    const { node } = await fromJsx(
      <select defaultValue="A">
        <option value="M">Monthly</option>
        <option value="A">Annual</option>
      </select>,
    );
    const [shown, monthly, annual] = children(node);

    expect(textOf(shown)).toBe("Annual");
    expect(monthly?.attributes).toEqual({ value: "M" });
    expect(annual?.attributes).toEqual({ value: "A", selected: "" });
  });

  test("a list box lays its options out", async () => {
    const { node } = await fromJsx(
      <select multiple defaultValue={["M", "A"]}>
        <option value="M">Monthly</option>
        <option value="A">Annual</option>
      </select>,
    );
    const options = children(node);

    expect(options).toHaveLength(2);
    expect(options.map((option) => option.attributes?.selected)).toEqual(["", ""]);
    expect(options[0]?.preset).toBe(defaultStylePresets.option);
  });

  test("a textarea's defaultValue is its text", async () => {
    const { node } = await fromJsx(<textarea name="notes" defaultValue="Two lines" />);

    expect(node).toEqual({
      type: "text",
      text: "Two lines",
      tagName: "textarea",
      attributes: { name: "notes", value: "Two lines" },
      preset: defaultStylePresets.textarea,
    } satisfies TextNode);
  });

  test("a push button shows its value", async () => {
    const { node: send } = await fromJsx(<input type="submit" value="Send" />);
    const { node: bare } = await fromJsx(<input type="reset" />);
    const { node: plain } = await fromJsx(<input type="text" value="x" />);
    const { node: markup } = fromHtml(`<input type="button" value="Go">`);

    expect(textOf(send)).toBe("Send");
    expect(textOf(bare)).toBe("Reset");
    expect(plain.type).toBe("container");
    expect(textOf(markup)).toBe("Go");
  });

  test("markup closes a select the same way", () => {
    const { node } = fromHtml(
      `<select><option label="Annual">A</option><option disabled>Weekly</option></select>`,
    );
    const [shown] = children(node as ContainerNode);

    expect(textOf(shown)).toBe("Annual");
  });
});
