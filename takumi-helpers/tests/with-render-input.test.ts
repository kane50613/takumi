import { expect, mock, test } from "bun:test";
import { withRenderInput } from "../src/renderer";
import type { FontLoader } from "../src/renderer";
import type { Node } from "../src/types";
import { fontBackend } from "./font-backend";

const bytes = () => new Uint8Array([1, 2, 3]);
const textNode = (text: string): Node => ({ type: "text", text });

const rangedFont = (name: string, unicodeRange: [number, number][]) => ({
  name,
  unicodeRange,
  data: mock(() => bytes()),
});

test("withRenderInput loads the subsets the text of the source uses", async () => {
  const { fonts } = fontBackend();
  const latin = rangedFont("Latin", [[0x41, 0x5a]]);
  const han = rangedFont("Han", [[0x4e00, 0x4e00]]);
  const call = mock((_options: unknown, _signal?: AbortSignal) => "rendered");
  const options: { fonts: FontLoader[]; width: number } = { fonts: [latin, han], width: 10 };

  expect(await withRenderInput(fonts, options, textNode("A"), call)).toBe("rendered");
  expect(latin.data).toHaveBeenCalledTimes(1);
  expect(han.data).not.toHaveBeenCalled();
  expect(call).toHaveBeenCalledWith(
    { width: 10, fontFamilies: ["Latin"], images: undefined },
    undefined,
  );
});

test("withRenderInput rejects before the call when the signal is aborted", async () => {
  const { fonts } = fontBackend();
  const call = mock(() => "rendered");

  await expect(
    withRenderInput(fonts, { signal: AbortSignal.abort() }, textNode("A"), call),
  ).rejects.toThrow();
  expect(call).not.toHaveBeenCalled();
});
