import { mkdir } from "node:fs/promises";
import { write } from "bun";
import { render } from "takumi-pdf";
import { invoice } from "./data";
import { InvoiceDocument } from "./invoice";
import { ReceiptDocument } from "./receipt";

await mkdir("output", { recursive: true });

const images = [
  {
    src: "logo.svg",
    data: new Uint8Array(await Bun.file("../../docs/public/logo.svg").arrayBuffer()),
  },
];

const invoicePdf = await render(<InvoiceDocument data={invoice} />, {
  size: "a4",
  margin: 48,
  images,
  footer: (
    <div tw="flex w-full justify-center text-[10px] text-[#6b7280]">
      Page <span className="pageNumber" /> of <span className="totalPages" />
    </div>
  ),
});

await write("output/invoice.pdf", invoicePdf);

const receiptPdf = await render(<ReceiptDocument data={invoice} />, {
  viewport: { width: 302 },
  images,
});

await write("output/receipt.pdf", receiptPdf);

console.log("Wrote output/invoice.pdf and output/receipt.pdf");
