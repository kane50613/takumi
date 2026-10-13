"use client";

import { useEffect, useRef, useState } from "react";

const SOURCE = "/images/hero/demo-card@2x.webp";
const COLUMNS = 192;
const ROWS = 101;
const PITCH = 0.068;
const RELIEF = 0.55;
const SCAN_MS = 1400;
const DROP_MS = 520;
const POINTER_RADIUS = 1.1;

/** `t` in 0..1, eased like `cubic-bezier(0.23, 1, 0.32, 1)`. */
function easeOut(t: number) {
  return 1 - (1 - Math.min(Math.max(t, 0), 1)) ** 4;
}

/** The card's pixels, downsampled to one RGBA sample per block. */
async function samplePixels() {
  const image = new Image();

  image.src = SOURCE;
  await image.decode();

  const canvas = document.createElement("canvas");

  canvas.width = COLUMNS;
  canvas.height = ROWS;

  const context = canvas.getContext("2d", { willReadFrequently: true });

  context?.drawImage(image, 0, 0, COLUMNS, ROWS);
  return context?.getImageData(0, 0, COLUMNS, ROWS).data;
}

/** A Takumi render as a field of physical pixels: they drop into place in scan order, and the pointer lifts them. */
export function PixelField() {
  const host = useRef<HTMLDivElement>(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    const element = host.current;

    if (!element) {
      return;
    }

    let disposed = false;
    let cleanup = () => {};

    void Promise.all([import("three"), samplePixels()]).then(([THREE, pixels]) => {
      if (disposed || !pixels) {
        return;
      }

      const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;
      const finePointer = matchMedia("(hover: hover) and (pointer: fine)").matches;
      const count = COLUMNS * ROWS;
      const renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
      const scene = new THREE.Scene();
      const camera = new THREE.PerspectiveCamera(30, 1, 0.1, 100);
      const field = new THREE.InstancedMesh(
        new THREE.BoxGeometry(PITCH * 0.92, PITCH * 0.92, 1),
        new THREE.MeshStandardMaterial({ roughness: 0.55, metalness: 0.05 }),
        count,
      );
      const key = new THREE.DirectionalLight(0xffffff, 2.4);
      const lamp = new THREE.PointLight(0xff4d4d, 0, 3.2, 1.6);
      const color = new THREE.Color();
      const matrix = new THREE.Matrix4();
      const position = new THREE.Vector3();
      const scale = new THREE.Vector3();
      const rotation = new THREE.Quaternion();
      const home = new Float32Array(count * 2);
      const depth = new Float32Array(count);
      const lift = new Float32Array(count);
      const liftVelocity = new Float32Array(count);

      for (let index = 0; index < count; index++) {
        const column = index % COLUMNS;
        const row = Math.floor(index / COLUMNS);
        const [r = 0, g = 0, b = 0] = pixels.subarray(index * 4, index * 4 + 3);
        const luminance = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255;

        home[index * 2] = (column - (COLUMNS - 1) / 2) * PITCH;
        home[index * 2 + 1] = ((ROWS - 1) / 2 - row) * PITCH;
        depth[index] = 0.06 + luminance * RELIEF;
        field.setColorAt(index, color.setRGB(r / 255, g / 255, b / 255, THREE.SRGBColorSpace));
      }

      key.position.set(-4, 6, 8);
      scene.add(field, key, new THREE.AmbientLight(0xffffff, 0.35), lamp);
      field.rotation.set(-0.42, 0.28, 0.06);
      camera.position.set(0, -0.2, 17);
      renderer.toneMapping = THREE.ACESFilmicToneMapping;
      renderer.outputColorSpace = THREE.SRGBColorSpace;
      renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
      element.appendChild(renderer.domElement);

      const resize = () => {
        const { width, height } = element.getBoundingClientRect();

        renderer.setSize(width, height, false);
        camera.aspect = width / height;
        camera.updateProjectionMatrix();
      };
      const resizeObserver = new ResizeObserver(resize);

      resizeObserver.observe(element);
      resize();

      const raycaster = new THREE.Raycaster();
      const plane = new THREE.Plane(new THREE.Vector3(0, 0, 1), 0);
      const pointerNdc = new THREE.Vector2(9, 9);
      const hit = new THREE.Vector3();
      const local = new THREE.Vector3(9e3, 9e3, 0);
      const onPointerMove = (event: PointerEvent) => {
        const rect = element.getBoundingClientRect();

        pointerNdc.set(
          ((event.clientX - rect.left) / rect.width) * 2 - 1,
          -((event.clientY - rect.top) / rect.height) * 2 + 1,
        );
      };
      const onPointerLeave = () => pointerNdc.set(9, 9);

      if (finePointer && !reduceMotion) {
        element.addEventListener("pointermove", onPointerMove, { passive: true });
        element.addEventListener("pointerleave", onPointerLeave);
      }

      let frame = 0;
      let visible = true;
      let start = performance.now();
      let last = start;

      const draw = (now: number) => {
        const dt = Math.min((now - last) / 1000, 1 / 30);
        const elapsed = reduceMotion ? Infinity : now - start;

        last = now;
        raycaster.setFromCamera(pointerNdc, camera);
        if (raycaster.ray.intersectPlane(plane.clone().applyMatrix4(field.matrixWorld), hit)) {
          local.copy(field.worldToLocal(hit));
        } else {
          local.set(9e3, 9e3, 0);
        }
        lamp.position.copy(field.localToWorld(hit.copy(local).setZ(1.4)));
        lamp.intensity = local.x < 9e2 ? 28 : 0;

        for (let index = 0; index < count; index++) {
          const x = home[index * 2] ?? 0;
          const y = home[index * 2 + 1] ?? 0;
          const distance = Math.hypot(x - local.x, y - local.y);
          const target = distance < POINTER_RADIUS ? (1 - distance / POINTER_RADIUS) ** 2 * 1.6 : 0;
          const settle = easeOut((elapsed - (index / count) * SCAN_MS) / DROP_MS);
          const blockDepth = depth[index] ?? 0;

          liftVelocity[index] =
            ((liftVelocity[index] ?? 0) + (target - (lift[index] ?? 0)) * 140 * dt) * (1 - 14 * dt);
          lift[index] = (lift[index] ?? 0) + (liftVelocity[index] ?? 0) * dt;
          position.set(x, y, blockDepth / 2 + (lift[index] ?? 0) + (1 - settle) * 6);
          scale.set(1, 1, Math.max(blockDepth * settle, 0.001));
          field.setMatrixAt(index, matrix.compose(position, rotation, scale));
        }

        field.instanceMatrix.needsUpdate = true;
        field.rotation.y = 0.28 + (pointerNdc.x < 2 ? pointerNdc.x * 0.04 : 0);
        renderer.render(scene, camera);
        frame = visible && !reduceMotion ? requestAnimationFrame(draw) : 0;
      };

      const resume = () => {
        if (frame === 0 && visible && !document.hidden && !reduceMotion) {
          last = performance.now();
          frame = requestAnimationFrame(draw);
        }
      };
      const intersection = new IntersectionObserver(([entry]) => {
        visible = entry?.isIntersecting ?? false;
        resume();
      });
      const onVisibility = () => {
        if (document.hidden) {
          cancelAnimationFrame(frame);
          frame = 0;
        } else {
          resume();
        }
      };

      intersection.observe(element);
      document.addEventListener("visibilitychange", onVisibility);
      start = performance.now();
      frame = requestAnimationFrame(draw);
      setReady(true);

      cleanup = () => {
        cancelAnimationFrame(frame);
        intersection.disconnect();
        resizeObserver.disconnect();
        element.removeEventListener("pointermove", onPointerMove);
        element.removeEventListener("pointerleave", onPointerLeave);
        document.removeEventListener("visibilitychange", onVisibility);
        field.geometry.dispose();
        field.material.dispose();
        field.dispose();
        renderer.dispose();
        renderer.domElement.remove();
      };
    });

    return () => {
      disposed = true;
      cleanup();
    };
  }, []);

  return (
    <div
      ref={host}
      aria-hidden
      data-ready={ready}
      className="absolute inset-0 opacity-0 transition-opacity duration-500 ease-out data-[ready=true]:opacity-100 [&>canvas]:size-full"
    />
  );
}
