import { expect, test } from "bun:test";
import { inspectPdf } from "./inspect-pdf";

/** Hand-written so the streams stay uncompressed and the offsets stay readable. */
function pdf(objects: string[]): Uint8Array {
  return new TextEncoder().encode(
    `%PDF-1.7\n${objects.map((object, index) => `${index + 1} 0 obj\n${object}\nendobj\n`).join("")}`,
  );
}

function streamObject(entries: string, data: string): string {
  return `<<${entries}/Length ${data.length}>>\nstream\n${data}\nendstream`;
}

// The font and its CMap come first so both stay at the same object number
// whatever else the fixture holds.
const FONT = "<</Type/Font/Subtype/Type0/ToUnicode 2 0 R>>";
const RESOURCES = "/Resources<</Font<</f0 1 0 R>>>>";

function cmap(entries: string): string {
  return streamObject("/Type/CMap", entries);
}

/** `<0001>` and `<0002>` spell "Hi", one word however many pages draw it. */
const HI = cmap("beginbfchar\n<0001><0048>\n<0002><0069>\nendbfchar");
const CONTENT = streamObject("", "BT /f0 12 Tf <00010002> Tj ET");

test("counts a shared content stream against both pages", async () => {
  const inspection = await inspectPdf(
    pdf([
      FONT,
      HI,
      "<</Type/Catalog/Pages 4 0 R>>",
      "<</Type/Pages/Count 2/Kids[5 0 R 6 0 R]>>",
      `<</Type/Page/Parent 4 0 R${RESOURCES}/Contents 7 0 R>>`,
      `<</Type/Page/Parent 4 0 R${RESOURCES}/Contents 7 0 R>>`,
      CONTENT,
    ]),
  );

  expect(inspection.pageText).toEqual([
    { blocks: 1, words: 1 },
    { blocks: 1, words: 1 },
  ]);
  expect(inspection.objects.find((object) => object.number === "7")?.label).toBe(
    "content stream, pages 1, 2",
  );
});

test("names the single page a content stream belongs to", async () => {
  const inspection = await inspectPdf(
    pdf([
      FONT,
      HI,
      "<</Type/Catalog/Pages 4 0 R>>",
      "<</Type/Pages/Count 1/Kids[5 0 R]>>",
      `<</Type/Page/Parent 4 0 R${RESOURCES}/Contents 6 0 R>>`,
      CONTENT,
    ]),
  );

  expect(inspection.pageText).toEqual([{ blocks: 1, words: 1 }]);
  expect(inspection.objects.find((object) => object.number === "6")?.label).toBe(
    "content stream, page 1",
  );
});

test("reads a content stream back through its ToUnicode CMap", async () => {
  const inspection = await inspectPdf(
    pdf([
      FONT,
      cmap(
        "beginbfchar\n<0001><0048>\n<0002><0069>\nendbfchar\nbeginbfrange\n<0003><0003><0021>\nendbfrange",
      ),
      "<</Type/Catalog/Pages 4 0 R>>",
      "<</Type/Pages/Count 1/Kids[5 0 R]>>",
      `<</Type/Page/Parent 4 0 R${RESOURCES}/Contents 6 0 R>>`,
      streamObject("", "BT /f0 12 Tf [(\\000\\001\\000\\002)-20(\\000\\003)] TJ ET"),
    ]),
  );

  expect(inspection.objects.find((object) => object.number === "6")?.text).toBe("Hi!");
});

test("steps a bfrange without breaking a surrogate pair", async () => {
  const inspection = await inspectPdf(
    pdf([
      FONT,
      cmap("beginbfrange\n<0001><0002><D83DDE00>\nendbfrange"),
      "<</Type/Catalog/Pages 4 0 R>>",
      "<</Type/Pages/Count 1/Kids[5 0 R]>>",
      `<</Type/Page/Parent 4 0 R${RESOURCES}/Contents 6 0 R>>`,
      streamObject("", "BT /f0 12 Tf <00010002> Tj ET"),
    ]),
  );

  expect(inspection.objects.find((object) => object.number === "6")?.text).toBe("😀😁");
});

test("keeps a stream byte that the endstream newline would have eaten", async () => {
  const inspection = await inspectPdf(
    pdf([
      "<</Type/Catalog/Pages 2 0 R>>",
      "<</Type/Pages/Count 1/Kids[3 0 R]>>",
      "<</Type/Page/Parent 2 0 R/Contents 4 0 R>>",
      streamObject("", "BT ET\n"),
    ]),
  );

  expect(inspection.objects.find((object) => object.number === "4")?.body).toBe("BT ET\n");
});
