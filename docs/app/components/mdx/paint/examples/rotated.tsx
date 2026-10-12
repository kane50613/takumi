import { paint } from "takumi-paint";

export const ROTATIONS = [0, 15, 30] as const;

export function paintRotated(degrees: number) {
  return paint(
    <div style={{ display: "flex", height: "100%", padding: 48, background: "#F7F3EC" }}>
      <div
        id="badge"
        style={{
          width: 160,
          height: 72,
          borderRadius: 12,
          background: "#8B5CF6",
          transform: `rotate(${degrees}deg)`,
        }}
      />
    </div>,
    { width: 320, height: 200 },
  );
}
