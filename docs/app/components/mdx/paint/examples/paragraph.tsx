import { paint } from "takumi-paint";

export const tree = await paint(
  <p style={{ width: 260, padding: 24, margin: 0, fontSize: 20, lineHeight: "30px" }}>
    Text arrives as <b>shaped runs</b> with their baselines.
  </p>,
  { width: 308 },
);
