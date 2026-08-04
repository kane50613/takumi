import { type Invoice, money, subtotal } from "./data";

const muted = "#687385";
const hairline = "1px solid #ebeef1";

function Meta({ label, value }: { label: string; value: string }) {
  return (
    <div style={{ display: "flex", gap: 12, fontSize: 12 }}>
      <span style={{ color: muted, width: 90 }}>{label}</span>
      <span>{value}</span>
    </div>
  );
}

function Party({ label, party }: { label?: string; party: Invoice["seller"] }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 2, fontSize: 12, width: 220 }}>
      <span style={{ fontWeight: 600, marginBottom: 4 }}>{label ?? party.name}</span>
      {label && <span>{party.name}</span>}
      <span style={{ color: muted }}>{party.address}</span>
      <span style={{ color: muted }}>{party.email}</span>
    </div>
  );
}

export function InvoiceDocument({ data }: { data: Invoice }) {
  const net = subtotal(data);
  const tax = net * data.taxRate;
  const total = net + tax;

  return (
    <div
      style={{ display: "flex", flexDirection: "column", gap: 36, width: "100%", color: "#30313d" }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start" }}>
        <div style={{ display: "flex", flexDirection: "column", gap: 16 }}>
          <span style={{ fontSize: 24, fontWeight: 600 }}>Invoice</span>
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            <Meta label="Invoice number" value={data.number} />
            <Meta label="Date of issue" value={data.issuedAt} />
            <Meta label="Date due" value={data.dueAt} />
          </div>
        </div>
        <img src="logo.svg" style={{ width: 40, height: 40 }} />
      </div>

      <div style={{ display: "flex", gap: 48 }}>
        <Party party={data.seller} />
        <Party label="Bill to" party={data.buyer} />
      </div>

      <span style={{ fontSize: 19, fontWeight: 600 }}>
        {money(total)} due {data.dueAt}
      </span>

      <div style={{ display: "flex", flexDirection: "column" }}>
        <div
          style={{
            display: "flex",
            gap: 12,
            paddingBottom: 8,
            borderBottom: hairline,
            fontSize: 11,
            fontWeight: 600,
            color: muted,
          }}
        >
          <span style={{ flex: 1 }}>Description</span>
          <span style={{ width: 50, textAlign: "right" }}>Qty</span>
          <span style={{ width: 100, textAlign: "right" }}>Unit price</span>
          <span style={{ width: 100, textAlign: "right" }}>Amount</span>
        </div>
        {data.items.map((item) => (
          <div
            key={item.description}
            style={{ display: "flex", gap: 12, paddingTop: 12, fontSize: 12, breakInside: "avoid" }}
          >
            <span style={{ flex: 1 }}>{item.description}</span>
            <span style={{ width: 50, textAlign: "right", color: muted }}>{item.quantity}</span>
            <span style={{ width: 100, textAlign: "right", color: muted }}>
              {money(item.unitPrice)}
            </span>
            <span style={{ width: 100, textAlign: "right" }}>
              {money(item.quantity * item.unitPrice)}
            </span>
          </div>
        ))}
      </div>

      <div style={{ display: "flex", justifyContent: "flex-end", breakInside: "avoid" }}>
        <div
          style={{ display: "flex", flexDirection: "column", width: 280, gap: 10, fontSize: 12 }}
        >
          <div style={{ display: "flex", justifyContent: "space-between" }}>
            <span style={{ color: muted }}>Subtotal</span>
            <span>{money(net)}</span>
          </div>
          <div style={{ display: "flex", justifyContent: "space-between" }}>
            <span style={{ color: muted }}>Tax ({data.taxRate * 100}%)</span>
            <span>{money(tax)}</span>
          </div>
          <div
            style={{
              display: "flex",
              justifyContent: "space-between",
              borderTop: hairline,
              paddingTop: 10,
              fontWeight: 600,
              fontSize: 13,
            }}
          >
            <span>Amount due</span>
            <span>{money(total)}</span>
          </div>
        </div>
      </div>

      <span style={{ fontSize: 11, color: muted }}>{data.notes}</span>
    </div>
  );
}
