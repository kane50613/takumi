import { describe, expect, it } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { evaluateCodeExports, renderReact } from "./evaluate";

const CODE = `export const options = { width: 100, height: 100 };
export default function Card() {
  return <div dangerouslySetInnerHTML={{ __html: '<img src=x onerror="alert(1)">' }} />;
}`;

describe("evaluated templates", () => {
  it("drops raw markup on its way to the preview", () => {
    const { default: component } = evaluateCodeExports(CODE, renderReact);
    const html = renderToStaticMarkup(renderReact.createElement(component));

    expect(html).toBe("<div></div>");
  });
});
