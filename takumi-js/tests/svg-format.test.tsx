import { expect, test } from "bun:test";
import { render, renderSvg } from "../src";
import ImageResponse from "../src/response";

const card = <div style={{ width: 100, height: 50, backgroundColor: "red" }}>Hi</div>;

test("render with format svg returns the SVG renderSvg draws, as UTF-8", async () => {
  const bytes = await render(card, { width: 100, height: 50, format: "svg" });

  expect(new TextDecoder().decode(bytes)).toBe(await renderSvg(card, { width: 100, height: 50 }));
});

test("ImageResponse sends SVG with its content type", async () => {
  const response = new ImageResponse(card, { width: 100, height: 50, format: "svg" });

  expect(response.headers.get("content-type")).toBe("image/svg+xml");
  expect(await response.text()).toStartWith("<svg");
});
