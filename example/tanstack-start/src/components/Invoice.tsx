type InvoiceProps = {
  id: number;
  items: { name: string; price: number }[];
};

export default function Invoice({ id, items }: InvoiceProps) {
  const total = items.reduce((sum, item) => sum + item.price, 0);

  return (
    <main style={{ padding: 48, fontFamily: "Geist" }}>
      <h1>Invoice {id}</h1>
      <table style={{ width: "100%", borderCollapse: "collapse" }}>
        <tbody>
          {items.map((item) => (
            <tr key={item.name}>
              <td style={{ padding: "8px 0", borderBottom: "1px solid #e5e5e5" }}>{item.name}</td>
              <td
                style={{ padding: "8px 0", borderBottom: "1px solid #e5e5e5", textAlign: "right" }}
              >
                ${item.price}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <p>Total: ${total}</p>
    </main>
  );
}
