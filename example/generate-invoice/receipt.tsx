import { type Invoice, money, subtotal } from "./data";

const muted = "#6b7280";

function Divider() {
  return <div style={{ borderBottom: "1px dashed #d1d5db" }} />;
}

function Row({ label, value, strong }: { label: string; value: string; strong?: boolean }) {
  return (
    <div
      style={{
        display: "flex",
        justifyContent: "space-between",
        fontSize: strong ? 15 : 11,
        fontWeight: strong ? 700 : 400,
        color: strong ? "#111827" : muted,
      }}
    >
      <span>{label}</span>
      <span style={{ color: "#111827" }}>{value}</span>
    </div>
  );
}

export function ReceiptDocument({ data }: { data: Invoice }) {
  const net = subtotal(data);

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 12,
        width: "100%",
        height: "100%",
        padding: 20,
        backgroundColor: "#ffffff",
        color: "#111827",
        fontSize: 11,
      }}
    >
      <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 6 }}>
        <img src="logo.svg" style={{ width: 32, height: 32 }} />
        <span
          style={{ fontSize: 15, fontWeight: 700, letterSpacing: 2, textTransform: "uppercase" }}
        >
          {data.seller.name}
        </span>
        <span style={{ color: muted, textAlign: "center", fontSize: 10 }}>
          {data.seller.address}
        </span>
      </div>

      <Divider />

      <div style={{ display: "flex", justifyContent: "space-between", color: muted, fontSize: 10 }}>
        <span>{data.issuedAt}</span>
        <span>{data.number}</span>
      </div>

      <Divider />

      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        {data.items.map((item) => (
          <div key={item.description} style={{ display: "flex", flexDirection: "column", gap: 1 }}>
            <span style={{ fontWeight: 500 }}>{item.description}</span>
            <div style={{ display: "flex", justifyContent: "space-between", color: muted }}>
              <span>
                {item.quantity} × {money(item.unitPrice)}
              </span>
              <span style={{ color: "#111827" }}>{money(item.quantity * item.unitPrice)}</span>
            </div>
          </div>
        ))}
      </div>

      <Divider />

      <div style={{ display: "flex", flexDirection: "column", gap: 5 }}>
        <Row label="Subtotal" value={money(net)} />
        <Row label={`Tax (${data.taxRate * 100}%)`} value={money(net * data.taxRate)} />
      </div>

      <Divider />

      <Row label="Total" value={money(net * (1 + data.taxRate))} strong />

      <Divider />

      <span style={{ textAlign: "center", color: muted, fontSize: 10, letterSpacing: 1 }}>
        THANK YOU FOR YOUR BUSINESS
      </span>
    </div>
  );
}
