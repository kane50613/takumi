import type { ReactNode } from "react";

export default function ProductCardTemplateV1({
  productName,
  price,
  description,
  image,
  brand,
}: {
  productName: ReactNode;
  price: ReactNode;
  description: ReactNode;
  image: ReactNode;
  brand: ReactNode;
}) {
  return (
    <div
      style={{
        display: "flex",
        width: "100%",
        height: "100%",
        backgroundColor: "#f3f4f6",
        padding: "40px",
        alignItems: "center",
        justifyContent: "center",
      }}
    >
      <div
        style={{
          display: "flex",
          width: "100%",
          height: "100%",
          backgroundColor: "white",
          borderRadius: "32px",
          overflow: "hidden",
          boxShadow: "0 20px 50px rgba(0,0,0,0.1)",
        }}
      >
        <div
          style={{
            flex: 1,
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            backgroundColor: "#e5e7eb",
            padding: "40px",
          }}
        >
          {image}
        </div>
        <div
          style={{
            flex: 1,
            display: "flex",
            flexDirection: "column",
            padding: "60px",
            justifyContent: "center",
          }}
        >
          <span
            style={{
              fontSize: 24,
              color: "#6b7280",
              fontWeight: 600,
              marginBottom: "16px",
              textTransform: "uppercase",
              letterSpacing: "0.05em",
            }}
          >
            {brand}
          </span>
          <h1
            style={{
              fontSize: 64,
              fontWeight: 900,
              color: "#111827",
              margin: "0 0 24px 0",
              lineHeight: 1.1,
            }}
          >
            {productName}
          </h1>
          <p
            style={{
              fontSize: 32,
              color: "#4b5563",
              lineHeight: 1.5,
              marginBottom: "40px",
            }}
          >
            {description}
          </p>
          <div
            style={{
              fontSize: 56,
              fontWeight: 800,
              color: "#2563eb",
            }}
          >
            {price}
          </div>
        </div>
      </div>
    </div>
  );
}
