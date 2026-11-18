import { expect, spyOn, test } from "bun:test";
import { Renderer } from "../src/export";

test("warns about an unsupported property in each style sheet that uses it", async () => {
  const warn = spyOn(console, "warn").mockImplementation(() => {});
  const renderer = new Renderer();
  const options = { width: 4, height: 4, format: "raw" } as const;

  try {
    await renderer.render("<div></div>", { ...options, css: ["div { takumi-test-property: 1 }"] });
    await renderer.render("<div></div>", {
      ...options,
      css: ["div {\n  color: red;\n  takumi-test-property: 2;\n}"],
    });
    // A warning logged on a worker thread prints on the JS thread's next turn.
    await new Promise((resolve) => setImmediate(resolve));

    const warnings = warn.mock.calls
      .map(([message]) => String(message))
      .filter((message) => message.includes("takumi-test-property"));

    expect(warnings).toEqual([
      "takumi: ignored the property `takumi-test-property` at line 1, column 28 of its style sheet or `style` attribute, since takumi does not support it. Check its spelling.",
      "takumi: ignored the property `takumi-test-property` at line 3, column 24 of its style sheet or `style` attribute, since takumi does not support it. Check its spelling.",
    ]);
  } finally {
    warn.mockRestore();
  }
});

test("renders when console.warn throws", async () => {
  const warn = spyOn(console, "warn").mockImplementation(() => {
    throw new Error("console.warn failed");
  });
  const renderer = new Renderer();

  try {
    const pixels = await renderer.render("<div></div>", {
      width: 4,
      height: 4,
      format: "raw",
      css: ["div { takumi-throwing-warn: 1 }"],
    });
    await new Promise((resolve) => setImmediate(resolve));

    expect(pixels.length).toBe(64);
    expect(warn).toHaveBeenCalled();
  } finally {
    warn.mockRestore();
  }
});
