/** The card's pixels as a field of blocks, in plain WebGL2: one instanced draw, colors read from the image in the vertex shader. */

const COLUMNS = 240;
const ROWS = 126;
const RISE_MS = 900;
const POINTER_RADIUS = 14;
const RISE_DELAY_MS = 450;
const FOV = 0.5;

const VERTEX = `#version 300 es
layout(location = 0) in vec3 position;
layout(location = 1) in vec3 normal;
uniform mat4 viewProjection;
uniform sampler2D card;
uniform vec2 grid;
uniform float lod;
uniform float rise;
uniform vec3 pointer;
out vec3 color;
out vec3 surface;
void main() {
  float id = float(gl_InstanceID);
  vec2 cell = vec2(mod(id, grid.x), floor(id / grid.x));
  color = textureLod(card, (cell + 0.5) / grid, lod).rgb;
  float luminance = dot(color, vec3(0.2126, 0.7152, 0.0722));
  vec2 center = (cell - (grid - 1.0) * 0.5) * vec2(1.0, -1.0);
  float near = 1.0 - smoothstep(0.0, ${POINTER_RADIUS.toFixed(1)}, distance(center, pointer.xy));
  float height = 0.2 + (0.3 + luminance * 6.0) * rise + near * near * pointer.z * 16.0;
  surface = normal;
  gl_Position = viewProjection * vec4(center + position.xy * 0.9, position.z * height, 1.0);
}`;

const FRAGMENT = `#version 300 es
precision mediump float;
in vec3 color;
in vec3 surface;
out vec4 fragment;
void main() {
  float light = 0.5 + 0.5 * max(dot(surface, normalize(vec3(-0.35, 0.5, 0.8))), 0.0);
  fragment = vec4(pow(color * light, vec3(1.0 / 2.2)), 1.0);
}`;

/** A unit block on z = 0..1, without its bottom face: position then normal for each vertex. */
function blockVertices() {
  const faces: [number[], number[][]][] = [
    [
      [0, 0, 1],
      [
        [-1, -1, 1],
        [1, -1, 1],
        [1, 1, 1],
        [-1, 1, 1],
      ],
    ],
    [
      [0, -1, 0],
      [
        [-1, -1, 0],
        [1, -1, 0],
        [1, -1, 1],
        [-1, -1, 1],
      ],
    ],
    [
      [0, 1, 0],
      [
        [1, 1, 0],
        [-1, 1, 0],
        [-1, 1, 1],
        [1, 1, 1],
      ],
    ],
    [
      [-1, 0, 0],
      [
        [-1, 1, 0],
        [-1, -1, 0],
        [-1, -1, 1],
        [-1, 1, 1],
      ],
    ],
    [
      [1, 0, 0],
      [
        [1, -1, 0],
        [1, 1, 0],
        [1, 1, 1],
        [1, -1, 1],
      ],
    ],
  ];
  const data: number[] = [];

  for (const [normal, corners] of faces) {
    for (const at of [0, 1, 2, 0, 2, 3]) {
      const [x = 0, y = 0, z = 0] = corners[at] ?? [];

      data.push(x * 0.5, y * 0.5, z, ...normal);
    }
  }

  return new Float32Array(data);
}

type Matrix = Float32Array;

function multiply(a: Matrix, b: Matrix): Matrix {
  const out = new Float32Array(16);

  for (let column = 0; column < 4; column++) {
    for (let row = 0; row < 4; row++) {
      let sum = 0;

      for (let k = 0; k < 4; k++) {
        sum += (a[k * 4 + row] ?? 0) * (b[column * 4 + k] ?? 0);
      }
      out[column * 4 + row] = sum;
    }
  }

  return out;
}

function perspective(aspect: number): Matrix {
  const f = 1 / Math.tan(FOV / 2);
  const near = 1;
  const far = 2000;

  return new Float32Array([
    f / aspect,
    0,
    0,
    0,
    0,
    f,
    0,
    0,
    0,
    0,
    (far + near) / (near - far),
    -1,
    0,
    0,
    (2 * far * near) / (near - far),
    0,
  ]);
}

/** Tilts the field's top away by `tilt` radians, then moves it `distance` from the camera. */
function view(tilt: number, distance: number): Matrix {
  const cos = Math.cos(-tilt);
  const sin = Math.sin(-tilt);

  return new Float32Array([1, 0, 0, 0, 0, cos, sin, 0, 0, -sin, cos, 0, 0, 0, -distance, 1]);
}

function compile(gl: WebGL2RenderingContext, type: number, source: string) {
  const shader = gl.createShader(type);

  if (!shader) {
    throw new Error("WebGL2 shader allocation failed");
  }
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    throw new Error(gl.getShaderInfoLog(shader) ?? "WebGL2 shader failed to compile");
  }

  return shader;
}

export type PixelRelief = {
  /** Raises the blocks when `active`, lays them flat otherwise. */
  setActive: (active: boolean) => void;
  dispose: () => void;
};

/** Draws `image` as blocks on `canvas`; returns `undefined` without WebGL2. */
export function createPixelRelief(
  canvas: HTMLCanvasElement,
  image: HTMLImageElement,
): PixelRelief | undefined {
  const gl = canvas.getContext("webgl2", {
    antialias: true,
    alpha: true,
    premultipliedAlpha: true,
  });

  if (!gl) {
    return undefined;
  }

  const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;
  const program = gl.createProgram();

  gl.attachShader(program, compile(gl, gl.VERTEX_SHADER, VERTEX));
  gl.attachShader(program, compile(gl, gl.FRAGMENT_SHADER, FRAGMENT));
  gl.linkProgram(program);
  gl.useProgram(program);

  const vertices = blockVertices();
  const buffer = gl.createBuffer();

  gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
  gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STATIC_DRAW);
  gl.enableVertexAttribArray(0);
  gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 24, 0);
  gl.enableVertexAttribArray(1);
  gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 24, 12);

  const texture = gl.createTexture();

  gl.bindTexture(gl.TEXTURE_2D, texture);
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.SRGB8_ALPHA8, gl.RGBA, gl.UNSIGNED_BYTE, image);
  gl.generateMipmap(gl.TEXTURE_2D);
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
  gl.enable(gl.DEPTH_TEST);
  gl.enable(gl.CULL_FACE);

  const uniform = (name: string) => gl.getUniformLocation(program, name);
  const viewProjection = uniform("viewProjection");
  const rise = uniform("rise");
  const pointerUniform = uniform("pointer");

  gl.uniform2f(uniform("grid"), COLUMNS, ROWS);
  gl.uniform1f(uniform("lod"), Math.log2(image.naturalWidth / COLUMNS));

  const pointer = { x: 0, y: 0, strength: 0, target: 0 };
  let active = false;
  let activeAt = 0;
  let level = 0;
  let last = performance.now();
  let frame = 0;
  let inverse: ((x: number, y: number) => { x: number; y: number } | undefined) | undefined;

  const draw = (now: number) => {
    const dt = Math.min(now - last, 50);

    last = now;
    // Waits for the card to settle from the paint stage before the blocks rise.
    const rising = active && now - activeAt > RISE_DELAY_MS;

    level = reduceMotion
      ? Number(active)
      : Math.min(Math.max(level + (rising ? dt : -dt) / RISE_MS, 0), 1);
    pointer.strength += (pointer.target - pointer.strength) * Math.min(dt / 120, 1);

    const eased = level < 0.5 ? 4 * level ** 3 : 1 - (-2 * level + 2) ** 3 / 2;
    const width = canvas.clientWidth;
    const height = canvas.clientHeight;
    const scale = Math.min(devicePixelRatio, 2);
    const tilt = eased * 0.7;
    const aspect = width / height;
    // At rest the field covers the card exactly; the canvas overhangs it so raised blocks fit.
    const fill = (canvas.parentElement?.clientWidth ?? width) / width;
    const distance = COLUMNS / (2 * Math.tan(FOV / 2) * aspect * fill);
    const matrix = multiply(perspective(aspect), view(tilt, distance));

    if (canvas.width !== Math.round(width * scale)) {
      canvas.width = Math.round(width * scale);
      canvas.height = Math.round(height * scale);
    }
    inverse = (x, y) => {
      const rayX = x * Math.tan(FOV / 2) * aspect;
      const rayY = y * Math.tan(FOV / 2);
      const along = distance / (1 - rayY * Math.tan(tilt));

      return along > 0 ? { x: rayX * along, y: (rayY * along) / Math.cos(tilt) } : undefined;
    };
    gl.viewport(0, 0, canvas.width, canvas.height);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
    gl.uniformMatrix4fv(viewProjection, false, matrix);
    gl.uniform1f(rise, eased);
    gl.uniform3f(pointerUniform, pointer.x, pointer.y, reduceMotion ? 0 : pointer.strength);
    gl.drawArraysInstanced(gl.TRIANGLES, 0, vertices.length / 6, COLUMNS * ROWS);

    const settled =
      (level === 0 || level === 1) &&
      Math.abs(pointer.target - pointer.strength) < 0.01 &&
      (!active || now - activeAt > RISE_DELAY_MS);

    frame = settled || document.hidden ? 0 : requestAnimationFrame(draw);
  };

  const resume = () => {
    if (frame === 0) {
      last = performance.now();
      frame = requestAnimationFrame(draw);
    }
  };

  const onPointer = (event: PointerEvent) => {
    const rect = canvas.getBoundingClientRect();
    const point = inverse?.(
      ((event.clientX - rect.left) / rect.width) * 2 - 1,
      -((event.clientY - rect.top) / rect.height) * 2 + 1,
    );

    if (!point || !active) {
      return;
    }
    pointer.x = point.x;
    pointer.y = point.y;
    pointer.target = 1;
    if (event.pointerType !== "mouse") {
      setTimeout(() => {
        pointer.target = 0;
        resume();
      }, 500);
    }
    resume();
  };
  const onLeave = () => {
    pointer.target = 0;
    resume();
  };

  canvas.addEventListener("pointermove", onPointer);
  canvas.addEventListener("pointerdown", onPointer);
  canvas.addEventListener("pointerleave", onLeave);
  document.addEventListener("visibilitychange", resume);
  resume();

  return {
    setActive(next) {
      if (next && !active) {
        activeAt = performance.now();
      }
      active = next;
      if (!next) {
        pointer.target = 0;
      }
      resume();
    },
    dispose() {
      cancelAnimationFrame(frame);
      canvas.removeEventListener("pointermove", onPointer);
      canvas.removeEventListener("pointerdown", onPointer);
      canvas.removeEventListener("pointerleave", onLeave);
      document.removeEventListener("visibilitychange", resume);
      gl.deleteTexture(texture);
      gl.deleteBuffer(buffer);
      gl.deleteProgram(program);
    },
  };
}
