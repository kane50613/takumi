import { create, globals } from "webgpu";

Object.assign(globalThis, globals);

export interface GlassRect {
  x: number;
  y: number;
  width: number;
  height: number;
  radius: number;
}

// Physically-based port of whynotmake-it/flutter_liquid_glass (GLSL -> WGSL):
// circular-arc height profile over a squircle SDF, Snell refraction through the
// surface normal, chromatic aberration, and background-tinted rim lighting.
const shader = /* wgsl */ `
struct Params {
  resolution: vec2f,
  center: vec2f,
  half_size: vec2f,
  radius: f32,
  thickness: f32,
}

const REFRACTIVE_INDEX = 1.5;
const CHROMATIC_ABERRATION = 0.3;
const LIGHT_DIRECTION = vec2f(-0.5547, -0.832);
const LIGHT_INTENSITY = 0.5;
const AMBIENT_STRENGTH = 0.3;
const SATURATION = 1.15;
const GLASS_TINT = vec4f(1.0, 1.0, 1.0, 0.06);
const FROST_BLUR_PX = 1.6;
const LUMA = vec3f(0.299, 0.587, 0.114);

@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> params: Params;

struct VsOut {
  @builtin(position) pos: vec4f,
  @location(0) uv: vec2f,
}

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VsOut {
  let p = array(vec2f(-1, -1), vec2f(3, -1), vec2f(-1, 3));
  var out: VsOut;
  out.pos = vec4f(p[i], 0, 1);
  out.uv = p[i] * vec2f(0.5, -0.5) + 0.5;
  return out;
}

fn sd_squircle(p: vec2f, b: vec2f, r: f32) -> f32 {
  let rr = min(r, min(b.x, b.y));
  let q = abs(p) - b + rr;
  let m = max(q, vec2f(0.0));
  return min(max(q.x, q.y), 0.0) + sqrt(m.x * m.x + m.y * m.y) - rr;
}

fn sdf(p: vec2f) -> f32 {
  return sd_squircle(p, params.half_size, params.radius);
}

fn surface_height(sd: f32, thickness: f32) -> f32 {
  if (sd >= 0.0) {
    return 0.0;
  }
  if (sd < -thickness) {
    return thickness;
  }
  let x = thickness + sd;
  return sqrt(max(0.0, thickness * thickness - x * x));
}

fn surface_normal(p: vec2f, sd: f32, thickness: f32) -> vec3f {
  let e = vec2f(1.0, 0.0);
  let grad = vec2f(sdf(p + e.xy) - sdf(p - e.xy), sdf(p + e.yx) - sdf(p - e.yx)) * 0.5;
  let n_cos = max(thickness + sd, 0.0) / thickness;
  let n_sin = sqrt(max(0.0, 1.0 - n_cos * n_cos));
  return normalize(vec3f(grad * n_cos, n_sin));
}

fn blurred(uv: vec2f, radius_px: f32) -> vec3f {
  let texel = radius_px / params.resolution;
  var acc = vec3f(0.0);
  for (var y = -1; y <= 1; y++) {
    for (var x = -1; x <= 1; x++) {
      acc += textureSampleLevel(src, samp, uv + vec2f(f32(x), f32(y)) * texel, 0.0).rgb;
    }
  }
  return acc / 9.0;
}

@fragment
fn fs(in: VsOut) -> @location(0) vec4f {
  let px = in.uv * params.resolution;
  let p = px - params.center;
  let d = sdf(p);
  let thickness = params.thickness;

  let background = textureSampleLevel(src, samp, in.uv, 0.0).rgb;
  let shadow = exp(-max(d, 0.0) / 30.0) * 0.18;
  let outside = background * (1.0 - shadow);

  let height = surface_height(d, thickness);
  let normal = surface_normal(p, d, thickness);

  // Snell refraction: vertical incident ray through the curved glass surface
  let incident = vec3f(0.0, 0.0, -1.0);
  let refr = refract(incident, normal, 1.0 / REFRACTIVE_INDEX);
  let ray_length = (height + thickness * 8.0) / max(0.001, abs(refr.z));
  let displacement = refr.xy * ray_length;

  let dispersion = CHROMATIC_ABERRATION * 0.5;
  let inv_res = 1.0 / params.resolution;
  let red = blurred(in.uv + displacement * (1.0 + dispersion) * inv_res, FROST_BLUR_PX).r;
  let green = blurred(in.uv + displacement * inv_res, FROST_BLUR_PX).g;
  let blue = blurred(in.uv + displacement * (1.0 - dispersion) * inv_res, FROST_BLUR_PX).b;
  let refracted = vec3f(red, green, blue);

  var color = GLASS_TINT.rgb * GLASS_TINT.a + refracted * (1.0 - GLASS_TINT.a);
  let luminance = dot(color, LUMA);
  color = clamp(mix(vec3f(luminance), color, SATURATION), vec3f(0.0), vec3f(1.0));

  // Rim lighting tinted by the background it refracts
  let normalized_height = height / thickness;
  let thickness_scale = clamp(40.0 / max(thickness, 1.0), 1.0, 4.0);
  let edge_threshold = mix(0.8, 0.5, 1.0 / thickness_scale);
  let edge_factor = 1.0 - smoothstep(0.0, edge_threshold, normalized_height);

  if (edge_factor > 0.01) {
    let normal_xy = normalize(normal.xy + vec2f(1e-5, 0.0));
    let main_light = max(0.0, dot(normal_xy, LIGHT_DIRECTION));
    let opposite_light = max(0.0, dot(normal_xy, -LIGHT_DIRECTION));
    let influence = main_light + opposite_light * 0.8;
    let brightness = (pow(influence, 1.5) * LIGHT_INTENSITY * 3.0 + AMBIENT_STRENGTH * 0.5) *
      edge_factor * thickness_scale * 0.8;

    let bg_luminance = dot(refracted, LUMA);
    let saturated_bg = mix(refracted, refracted / max(bg_luminance, 0.001), 0.8);
    let colorfulness = length(refracted - vec3f(bg_luminance));
    let highlight = mix(vec3f(1.0), saturated_bg, clamp(colorfulness + 0.5, 0.5, 1.0));

    color = mix(color, highlight, clamp(brightness, 0.0, 1.0));
  }

  let t = smoothstep(1.0, -1.0, d);
  return vec4f(mix(outside, color, t), 1.0);
}
`;

export async function applyLiquidGlass(
  pixels: Uint8Array,
  width: number,
  height: number,
  glass: GlassRect,
  thickness = 24,
): Promise<Uint8Array> {
  const gpu = create([]);
  const adapter = await gpu.requestAdapter();

  if (!adapter) {
    throw new Error("No WebGPU adapter available");
  }

  const device = await adapter.requestDevice();

  const source = device.createTexture({
    size: [width, height],
    format: "rgba8unorm",
    usage: GPUTextureUsage.TEXTURE_BINDING | GPUTextureUsage.COPY_DST,
  });

  device.queue.writeTexture({ texture: source }, pixels, { bytesPerRow: width * 4 }, [
    width,
    height,
  ]);

  const target = device.createTexture({
    size: [width, height],
    format: "rgba8unorm",
    usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
  });

  const params = new Float32Array([
    width,
    height,
    glass.x + glass.width / 2,
    glass.y + glass.height / 2,
    glass.width / 2,
    glass.height / 2,
    glass.radius,
    thickness,
  ]);
  const uniform = device.createBuffer({
    size: params.byteLength,
    usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST,
  });

  device.queue.writeBuffer(uniform, 0, params);

  const module = device.createShaderModule({ code: shader });
  const pipeline = device.createRenderPipeline({
    layout: "auto",
    vertex: { module, entryPoint: "vs" },
    fragment: { module, entryPoint: "fs", targets: [{ format: "rgba8unorm" }] },
  });

  const bindGroup = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [
      { binding: 0, resource: source.createView() },
      { binding: 1, resource: device.createSampler({ magFilter: "linear", minFilter: "linear" }) },
      { binding: 2, resource: { buffer: uniform } },
    ],
  });

  // copyTextureToBuffer requires bytesPerRow aligned to 256
  const bytesPerRow = Math.ceil((width * 4) / 256) * 256;
  const readback = device.createBuffer({
    size: bytesPerRow * height,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  });

  const encoder = device.createCommandEncoder();
  const pass = encoder.beginRenderPass({
    colorAttachments: [{ view: target.createView(), loadOp: "clear", storeOp: "store" }],
  });

  pass.setPipeline(pipeline);
  pass.setBindGroup(0, bindGroup);
  pass.draw(3);
  pass.end();
  encoder.copyTextureToBuffer({ texture: target }, { buffer: readback, bytesPerRow }, [
    width,
    height,
  ]);
  device.queue.submit([encoder.finish()]);

  await readback.mapAsync(GPUMapMode.READ);

  const padded = new Uint8Array(readback.getMappedRange());
  const out = new Uint8Array(width * height * 4);

  for (let y = 0; y < height; y++) {
    out.set(padded.subarray(y * bytesPerRow, y * bytesPerRow + width * 4), y * width * 4);
  }

  readback.unmap();
  device.destroy();

  return out;
}
