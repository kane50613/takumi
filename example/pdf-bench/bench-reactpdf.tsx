import { interFonts } from "./fonts";
import { benchmark } from "./harness";
import { items, total } from "./invoice-data";

const interPaths = await interFonts();

const t0 = performance.now();
const { Document, Page, Text, View, StyleSheet, Font, pdf } = await import("@react-pdf/renderer");

Font.register({
  family: "Inter",
  fonts: [{ src: interPaths.regular }, { src: interPaths.bold, fontWeight: 700 }],
});
Font.registerHyphenationCallback((word) => [word]);

// react-pdf styles are in pt (1px at 96 dpi = 0.75pt); values mirror the px
// used by the takumi and Puppeteer harnesses.
const styles = StyleSheet.create({
  page: { padding: 36, fontSize: 9.75, fontFamily: "Inter", color: "#111827" },
  header: {
    flexDirection: "row",
    justifyContent: "space-between",
    borderBottomWidth: 0.75,
    borderBottomColor: "#d1d5db",
    paddingBottom: 12,
    marginBottom: 12,
  },
  title: { fontSize: 18, fontWeight: 700 },
  row: { flexDirection: "row", paddingVertical: 3 },
  description: { flexGrow: 1, flexShrink: 1, flexBasis: 0, paddingRight: 12 },
  qty: { width: 24, textAlign: "right" },
  price: { width: 67.5, textAlign: "right" },
  totalRow: {
    flexDirection: "row",
    justifyContent: "space-between",
    borderTopWidth: 0.75,
    borderTopColor: "#d1d5db",
    marginTop: 12,
    paddingTop: 6,
    fontWeight: 700,
  },
  pageNumber: {
    position: "absolute",
    bottom: 15,
    left: 0,
    right: 0,
    textAlign: "center",
    fontSize: 7.5,
    color: "#6b7280",
  },
});

function Invoice() {
  return (
    <Document>
      <Page size="A4" style={styles.page}>
        <View style={styles.header}>
          <Text style={styles.title}>Invoice INV-2026-001</Text>
          <Text>Due August 31, 2026</Text>
        </View>
        {items.map((item, i) => (
          <View key={i} style={styles.row} wrap={false}>
            <Text style={styles.description}>{item.description}</Text>
            <Text style={styles.qty}>{item.qty}</Text>
            <Text style={styles.price}>${(item.qty * item.unit).toFixed(2)}</Text>
          </View>
        ))}
        <View style={styles.totalRow}>
          <Text>Total</Text>
          <Text>${total.toFixed(2)}</Text>
        </View>
        <Text
          style={styles.pageNumber}
          render={({ pageNumber, totalPages }) => `Page ${pageNumber} of ${totalPages}`}
          fixed
        />
      </Page>
    </Document>
  );
}

async function renderOnce(): Promise<Uint8Array> {
  const buffer = await pdf(<Invoice />).toBuffer();
  const chunks: Buffer[] = [];
  for await (const chunk of buffer as unknown as AsyncIterable<Buffer>) {
    chunks.push(chunk);
  }
  return new Uint8Array(Buffer.concat(chunks));
}

await benchmark("@react-pdf/renderer", t0, renderOnce, "out-reactpdf.pdf");
