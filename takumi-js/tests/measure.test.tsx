import { expect, test } from "bun:test";
import { measure } from "../src";

test("measure applies css like render", async () => {
  const measured = await measure(<div className="box" />, {
    css: ".box { width: 120px; height: 40px; }",
  });

  expect([measured.width, measured.height]).toEqual([120, 40]);
});

test("measure takes an html string", async () => {
  const measured = await measure('<div style="width: 30px; height: 20px"></div>');

  expect([measured.width, measured.height]).toEqual([30, 20]);
});
