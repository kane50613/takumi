import { describe, expect, it } from "bun:test";
import { compressCode, decompressCode } from "./share";

describe("shared snippets", () => {
  it("round-trips code through the URL form", async () => {
    const code = "export default function Card() { return <div>hi</div>; }";

    expect(await decompressCode(await compressCode(code))).toBe(code);
  });

  it("refuses a snippet that expands past the cap", async () => {
    const bomb = await compressCode("x".repeat(1024 * 1024));

    await expect(decompressCode(bomb)).rejects.toThrow(/larger than the playground accepts/);
  });
});
