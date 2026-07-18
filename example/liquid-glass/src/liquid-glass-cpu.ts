import type { GlassRect } from "./liquid-glass.ts";

// Same math as the WGSL shader in liquid-glass.ts, run per pixel on the CPU.
// Proves the effect needs no GPU: it is pure per-pixel arithmetic.

const REFRACTIVE_INDEX = 1.5;
const CHROMATIC_ABERRATION = 0.3;
const LIGHT_DIRECTION_X = -0.5547;
const LIGHT_DIRECTION_Y = -0.832;
const LIGHT_INTENSITY = 0.5;
const AMBIENT_STRENGTH = 0.3;
const SATURATION = 1.15;
const GLASS_TINT_ALPHA = 0.06;
const FROST_BLUR_PX = 1.6;

function clamp(x: number, lo: number, hi: number) {
  return Math.min(Math.max(x, lo), hi);
}

function smoothstep(edge0: number, edge1: number, x: number) {
  const t = clamp((x - edge0) / (edge1 - edge0), 0, 1);

  return t * t * (3 - 2 * t);
}

function sdSquircle(px: number, py: number, bx: number, by: number, r: number) {
  const rr = Math.min(r, Math.min(bx, by));
  const qx = Math.abs(px) - bx + rr;
  const qy = Math.abs(py) - by + rr;
  const mx = Math.max(qx, 0);
  const my = Math.max(qy, 0);

  return Math.min(Math.max(qx, qy), 0) + Math.hypot(mx, my) - rr;
}

export function applyLiquidGlassCpu(
  pixels: Uint8Array,
  width: number,
  height: number,
  glass: GlassRect,
  thickness = 24,
): Uint8Array {
  const out = new Uint8Array(width * height * 4);
  const centerX = glass.x + glass.width / 2;
  const centerY = glass.y + glass.height / 2;
  const halfW = glass.width / 2;
  const halfH = glass.height / 2;
  const { radius } = glass;

  const sdf = (px: number, py: number) => sdSquircle(px, py, halfW, halfH, radius);
  const at = (index: number) => pixels[index] ?? 0;

  // Bilinear sample of one channel, clamp-to-edge, in 0..1
  const sample = (x: number, y: number, channel: number) => {
    const fx = clamp(x - 0.5, 0, width - 1);
    const fy = clamp(y - 0.5, 0, height - 1);
    const x0 = Math.floor(fx);
    const y0 = Math.floor(fy);
    const x1 = Math.min(x0 + 1, width - 1);
    const y1 = Math.min(y0 + 1, height - 1);
    const tx = fx - x0;
    const ty = fy - y0;
    const row0 = y0 * width;
    const row1 = y1 * width;
    const a = at((row0 + x0) * 4 + channel);
    const b = at((row0 + x1) * 4 + channel);
    const c = at((row1 + x0) * 4 + channel);
    const d = at((row1 + x1) * 4 + channel);

    return (a + (b - a) * tx + (c - a + (a - b + d - c) * tx) * ty) / 255;
  };

  const blurred = (x: number, y: number, radiusPx: number, channel: number) => {
    let acc = 0;

    for (let dy = -1; dy <= 1; dy++) {
      for (let dx = -1; dx <= 1; dx++) {
        acc += sample(x + dx * radiusPx, y + dy * radiusPx, channel);
      }
    }

    return acc / 9;
  };

  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = (y * width + x) * 4;
      const px = x + 0.5;
      const py = y + 0.5;
      const d = sdf(px - centerX, py - centerY);

      if (d >= 1) {
        const shadow = Math.exp(-d / 30) * 0.18;
        const k = 1 - shadow;

        out[i] = at(i) * k;
        out[i + 1] = at(i + 1) * k;
        out[i + 2] = at(i + 2) * k;
        out[i + 3] = 255;
        continue;
      }

      const h =
        d >= 0
          ? 0
          : d < -thickness
            ? thickness
            : Math.sqrt(Math.max(0, thickness * thickness - (thickness + d) ** 2));

      const gx = (sdf(px - centerX + 1, py - centerY) - sdf(px - centerX - 1, py - centerY)) * 0.5;
      const gy = (sdf(px - centerX, py - centerY + 1) - sdf(px - centerX, py - centerY - 1)) * 0.5;
      const nCos = Math.max(thickness + d, 0) / thickness;
      const nSin = Math.sqrt(Math.max(0, 1 - nCos * nCos));
      const nLen = Math.hypot(gx * nCos, gy * nCos, nSin);
      const nx = (gx * nCos) / nLen;
      const ny = (gy * nCos) / nLen;
      const nz = nSin / nLen;

      // refract((0,0,-1), n, eta): t = eta*i + (eta*cos_i - cos_t)*n
      const eta = 1 / REFRACTIVE_INDEX;
      const cosI = nz;
      const k = 1 - eta * eta * (1 - cosI * cosI);
      const scale = eta * cosI - Math.sqrt(Math.max(0, k));
      const rx = scale * nx;
      const ry = scale * ny;
      const rz = -eta + scale * nz;

      const rayLength = (h + thickness * 8) / Math.max(0.001, Math.abs(rz));
      const dispX = rx * rayLength;
      const dispY = ry * rayLength;

      const dispersion = CHROMATIC_ABERRATION * 0.5;
      const red = blurred(
        px + dispX * (1 + dispersion),
        py + dispY * (1 + dispersion),
        FROST_BLUR_PX,
        0,
      );
      const green = blurred(px + dispX, py + dispY, FROST_BLUR_PX, 1);
      const blue = blurred(
        px + dispX * (1 - dispersion),
        py + dispY * (1 - dispersion),
        FROST_BLUR_PX,
        2,
      );

      let r = GLASS_TINT_ALPHA + red * (1 - GLASS_TINT_ALPHA);
      let g = GLASS_TINT_ALPHA + green * (1 - GLASS_TINT_ALPHA);
      let b = GLASS_TINT_ALPHA + blue * (1 - GLASS_TINT_ALPHA);

      const luminance = 0.299 * r + 0.587 * g + 0.114 * b;

      r = clamp(luminance + (r - luminance) * SATURATION, 0, 1);
      g = clamp(luminance + (g - luminance) * SATURATION, 0, 1);
      b = clamp(luminance + (b - luminance) * SATURATION, 0, 1);

      const normalizedHeight = h / thickness;
      const thicknessScale = clamp(40 / Math.max(thickness, 1), 1, 4);
      const edgeThreshold = 0.8 + (0.5 - 0.8) * (1 / thicknessScale);
      const edgeFactor = 1 - smoothstep(0, edgeThreshold, normalizedHeight);

      if (edgeFactor > 0.01) {
        const nxyLen = Math.hypot(nx + 1e-5, ny);
        const ux = (nx + 1e-5) / nxyLen;
        const uy = ny / nxyLen;
        const mainLight = Math.max(0, ux * LIGHT_DIRECTION_X + uy * LIGHT_DIRECTION_Y);
        const oppositeLight = Math.max(0, -(ux * LIGHT_DIRECTION_X + uy * LIGHT_DIRECTION_Y));
        const influence = mainLight + oppositeLight * 0.8;
        const brightness = clamp(
          (influence ** 1.5 * LIGHT_INTENSITY * 3 + AMBIENT_STRENGTH * 0.5) *
            edgeFactor *
            thicknessScale *
            0.8,
          0,
          1,
        );

        const bgLuminance = 0.299 * red + 0.587 * green + 0.114 * blue;
        const inv = 1 / Math.max(bgLuminance, 0.001);
        const satR = red + (red * inv - red) * 0.8;
        const satG = green + (green * inv - green) * 0.8;
        const satB = blue + (blue * inv - blue) * 0.8;
        const colorfulness = Math.hypot(red - bgLuminance, green - bgLuminance, blue - bgLuminance);
        const colorMix = clamp(colorfulness + 0.5, 0.5, 1);
        const hr = 1 + (satR - 1) * colorMix;
        const hg = 1 + (satG - 1) * colorMix;
        const hb = 1 + (satB - 1) * colorMix;

        r += (hr - r) * brightness;
        g += (hg - g) * brightness;
        b += (hb - b) * brightness;
      }

      const shadow = Math.exp(-Math.max(d, 0) / 30) * 0.18;
      const t = smoothstep(1, -1, d);
      const outR = (at(i) / 255) * (1 - shadow);
      const outG = (at(i + 1) / 255) * (1 - shadow);
      const outB = (at(i + 2) / 255) * (1 - shadow);

      out[i] = clamp(outR + (r - outR) * t, 0, 1) * 255;
      out[i + 1] = clamp(outG + (g - outG) * t, 0, 1) * 255;
      out[i + 2] = clamp(outB + (b - outB) * t, 0, 1) * 255;
      out[i + 3] = 255;
    }
  }

  return out;
}
