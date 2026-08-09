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

const CONTENT = streamObject("", "BT /f0 12 Tf (hi) Tj ET");

test("counts a shared content stream against both pages", async () => {
  const inspection = await inspectPdf(
    pdf([
      "<</Type/Catalog/Pages 2 0 R>>",
      "<</Type/Pages/Count 2/Kids[3 0 R 4 0 R]>>",
      "<</Type/Page/Parent 2 0 R/Contents 5 0 R>>",
      "<</Type/Page/Parent 2 0 R/Contents 5 0 R>>",
      CONTENT,
    ]),
  );

  expect(inspection.pageText).toEqual([
    { blocks: 1, words: 0 },
    { blocks: 1, words: 0 },
  ]);
  expect(inspection.objects.find((object) => object.number === "5")?.label).toBe(
    "content stream, pages 1, 2",
  );
});

test("names the single page a content stream belongs to", async () => {
  const inspection = await inspectPdf(
    pdf([
      "<</Type/Catalog/Pages 2 0 R>>",
      "<</Type/Pages/Count 1/Kids[3 0 R]>>",
      "<</Type/Page/Parent 2 0 R/Contents 4 0 R>>",
      CONTENT,
    ]),
  );

  expect(inspection.pageText).toEqual([{ blocks: 1, words: 0 }]);
  expect(inspection.objects.find((object) => object.number === "4")?.label).toBe(
    "content stream, page 1",
  );
});

test("reads a content stream back through its ToUnicode CMap", async () => {
  const inspection = await inspectPdf(
    pdf([
      "<</Type/Catalog/Pages 2 0 R>>",
      "<</Type/Pages/Count 1/Kids[3 0 R]>>",
      "<</Type/Page/Parent 2 0 R/Resources<</Font<</f0 5 0 R>>>>/Contents 4 0 R>>",
      streamObject("", "BT /f0 12 Tf [(\\000\\001\\000\\002)-20(\\000\\003)] TJ ET"),
      "<</Type/Font/Subtype/Type0/ToUnicode 6 0 R>>",
      streamObject(
        "/Type/CMap",
        "beginbfchar\n<0001><0048>\n<0002><0069>\nendbfchar\nbeginbfrange\n<0003><0003><0021>\nendbfrange",
      ),
    ]),
  );

  expect(inspection.objects.find((object) => object.number === "4")?.text).toBe("Hi!");
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
