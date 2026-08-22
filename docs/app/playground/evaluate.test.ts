import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { renderReact } from "./evaluate";

describe("the React a template renders with", () => {
  it("mirrors tw into className for the preview", () => {
    const element = renderReact.createElement("div", { tw: "flex" });

    expect(renderToStaticMarkup(element)).toBe('<div tw="flex" class="flex"></div>');
  });
});
