import { items, total } from "./invoice-data";

const t0 = performance.now();
const { Document, Page, Text, View, StyleSheet, pdf } = await import("@react-pdf/renderer");

const styles = StyleSheet.create({
  page: { padding: 48, fontSize: 11 },
  header: {
    flexDirection: "row",
    justifyContent: "space-between",
    borderBottomWidth: 1,
    borderBottomColor: "#d1d5db",
    paddingBottom: 8,
    marginBottom: 8,
  },
  title: { fontSize: 20, fontWeight: 700 },
  row: { flexDirection: "row", justifyContent: "space-between", paddingVertical: 2 },
  description: { width: "80%" },
  totalRow: {
    flexDirection: "row",
    justifyContent: "space-between",
    borderTopWidth: 1,
    borderTopColor: "#d1d5db",
    marginTop: 8,
    paddingTop: 4,
    fontWeight: 700,
  },
  pageNumber: {
    position: "absolute",
    bottom: 20,
    left: 0,
    right: 0,
    textAlign: "center",
    fontSize: 8,
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
            <Text>{item.qty}</Text>
            <Text>${(item.qty * item.unit).toFixed(2)}</Text>
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

const first = await renderOnce();
const coldMs = performance.now() - t0;

const times: number[] = [];
for (let i = 0; i < 20; i++) {
  const start = performance.now();
  await renderOnce();
  times.push(performance.now() - start);
}
times.sort((a, b) => a - b);

await Bun.write("out-reactpdf.pdf", first);
console.log(
  JSON.stringify({
    engine: "@react-pdf/renderer",
    coldMs: Math.round(coldMs),
    warmMedianMs: Math.round(times[10]!),
    bytes: first.byteLength,
  }),
);
