import { type Invoice, money, subtotal } from "./data";

const muted = "#6b7280";
const hairline = "1px solid #e5e7eb";

const columns = [
  { label: "Description", flex: 1, align: "left" },
  { label: "Qty", width: 60, align: "right" },
  { label: "Unit price", width: 110, align: "right" },
  { label: "Amount", width: 110, align: "right" },
] as const;

function Party({ label, party }: { label: string; party: Invoice["seller"] }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 3, fontSize: 12 }}>
      <span
        style={{
          fontWeight: 700,
          textTransform: "uppercase",
          letterSpacing: 1,
          color: muted,
          fontSize: 9,
        }}
      >
        {label}
      </span>
      <span style={{ fontWeight: 700, fontSize: 14 }}>{party.name}</span>
      <span style={{ color: muted }}>{party.address}</span>
      <span style={{ color: muted }}>{party.email}</span>
    </div>
  );
}

export function InvoiceDocument({ data }: { data: Invoice }) {
  const net = subtotal(data);
  const tax = net * data.taxRate;

  return (
    <div
      style={{ display: "flex", flexDirection: "column", gap: 32, width: "100%", color: "#111827" }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <img src="logo.svg" style={{ width: 36, height: 36 }} />
          <div style={{ display: "flex", flexDirection: "column" }}>
            <span style={{ fontSize: 26, fontWeight: 700, letterSpacing: -0.5 }}>Invoice</span>
            <span style={{ fontSize: 12, color: muted }}>{data.number}</span>
          </div>
        </div>
        <div
          style={{
            display: "flex",
            flexDirection: "column",
            alignItems: "flex-end",
            fontSize: 12,
            gap: 2,
            color: muted,
          }}
        >
          <span>
            Issued <span style={{ color: "#111827", fontWeight: 700 }}>{data.issuedAt}</span>
          </span>
          <span>
            Due <span style={{ color: "#111827", fontWeight: 700 }}>{data.dueAt}</span>
          </span>
        </div>
      </div>

      <div style={{ display: "flex", gap: 64 }}>
        <Party label="From" party={data.seller} />
        <Party label="Bill to" party={data.buyer} />
      </div>

      <div style={{ display: "flex", flexDirection: "column" }}>
        <div
          style={{
            display: "flex",
            gap: 12,
            padding: "8px 0",
            borderBottom: "1px solid #111827",
            fontSize: 9,
            fontWeight: 700,
            textTransform: "uppercase",
            letterSpacing: 1,
            color: muted,
          }}
        >
          {columns.map((column) => (
            <span
              key={column.label}
              style={{
                flex: "flex" in column ? column.flex : undefined,
                width: "width" in column ? column.width : undefined,
                textAlign: column.align,
              }}
            >
              {column.label}
            </span>
          ))}
        </div>
        {data.items.map((item) => (
          <div
            key={item.description}
            style={{
              display: "flex",
              gap: 12,
              padding: "12px 0",
              borderBottom: hairline,
              fontSize: 12,
              breakInside: "avoid",
            }}
          >
            <span style={{ flex: 1, fontWeight: 500 }}>{item.description}</span>
            <span style={{ width: 60, textAlign: "right", color: muted }}>{item.quantity}</span>
            <span style={{ width: 110, textAlign: "right", color: muted }}>
              {money(item.unitPrice)}
            </span>
            <span style={{ width: 110, textAlign: "right" }}>
              {money(item.quantity * item.unitPrice)}
            </span>
          </div>
        ))}
      </div>

      <div style={{ display: "flex", justifyContent: "flex-end", breakInside: "avoid" }}>
        <div style={{ display: "flex", flexDirection: "column", width: 260, gap: 8 }}>
          <div
            style={{ display: "flex", justifyContent: "space-between", fontSize: 12, color: muted }}
          >
            <span>Subtotal</span>
            <span style={{ color: "#111827" }}>{money(net)}</span>
          </div>
          <div
            style={{ display: "flex", justifyContent: "space-between", fontSize: 12, color: muted }}
          >
            <span>Tax ({data.taxRate * 100}%)</span>
            <span style={{ color: "#111827" }}>{money(tax)}</span>
          </div>
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              alignItems: "center",
              borderTop: "1px solid #111827",
              paddingTop: 8,
              fontSize: 14,
              fontWeight: 700,
            }}
          >
            <span>Total due</span>
            <span style={{ fontSize: 16 }}>{money(net + tax)}</span>
          </div>
        </div>
      </div>

      <span style={{ fontSize: 11, color: muted }}>{data.notes}</span>
    </div>
  );
}
