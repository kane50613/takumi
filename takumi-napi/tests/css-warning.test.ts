import { expect, spyOn, test } from "bun:test";
import { Renderer } from "../src/export";

test("warns once about an unsupported property, however many renders use it", async () => {
  const warn = spyOn(console, "warn").mockImplementation(() => {});
  const renderer = new Renderer();
  const options = { width: 4, height: 4, format: "raw" } as const;

  try {
    await renderer.render("<div></div>", { ...options, css: ["div { takumi-test-property: 1 }"] });
    await renderer.render("<div></div>", {
      ...options,
      css: ["div {\n  color: red;\n  takumi-test-property: 2;\n}"],
    });

    const warnings = warn.mock.calls
      .map(([message]) => String(message))
      .filter((message) => message.includes("takumi-test-property"));

    expect(warnings).toEqual([
      "takumi: ignored the property `takumi-test-property` at line 1, column 28 of its style sheet or `style` attribute, since takumi does not support it. Check its spelling.",
    ]);
  } finally {
    warn.mockRestore();
  }
});
