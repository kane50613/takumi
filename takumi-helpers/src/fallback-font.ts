import geist from "../../assets/fonts/geist/geist-latin-wght-300-800.ttf";

/** `base64` decoded without `Buffer` or `Uint8Array.fromBase64`, which some runtimes lack. */
function decode(base64: string) {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);

  for (let index = 0; index < binary.length; index++) {
    bytes[index] = binary.charCodeAt(index);
  }

  return bytes;
}

/** Geist, Latin, weights 300 to 800: the last-resort font the wasm renderers start with. */
export const fallbackFont: Uint8Array | undefined = decode(geist);
