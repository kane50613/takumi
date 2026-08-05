export const items = Array.from({ length: 80 }, (_, i) => ({
  description: `Line item ${i + 1}: professional services rendered during the billing period`,
  qty: (i % 4) + 1,
  unit: 120 + i * 7,
}));

export const total = items.reduce((sum, item) => sum + item.qty * item.unit, 0);
