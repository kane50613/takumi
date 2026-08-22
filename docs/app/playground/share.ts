function uint8ToBase64(uint8: Uint8Array): string {
  if (typeof uint8.toBase64 === "function") return uint8.toBase64();

  let binary = "";
  for (const byte of uint8) {
    binary += String.fromCharCode(byte);
  }
  return btoa(binary);
}

function base64ToUint8(base64: string): Uint8Array<ArrayBuffer> {
  if (typeof Uint8Array.fromBase64 === "function") {
    return Uint8Array.fromBase64(base64);
  }

  const binary = atob(base64);
  return Uint8Array.from(binary, (char) => char.charCodeAt(0));
}

export async function compressCode(code: string) {
  const blob = new Blob([code]);

  const stream = blob.stream();
  const compressedStream = stream.pipeThrough(new CompressionStream("gzip"));

  const compressedArrayBuffer = await new Response(compressedStream).arrayBuffer();
  const compressedBytes = new Uint8Array(compressedArrayBuffer);

  return uint8ToBase64(compressedBytes);
}

/** A short link can hold a gzip stream that expands past what a tab can edit. */
const MAX_CODE_BYTES = 512 * 1024;

export async function decompressCode(base64: string) {
  const blob = new Blob([base64ToUint8(base64)]);
  const reader = blob.stream().pipeThrough(new DecompressionStream("gzip")).getReader();
  const decoder = new TextDecoder();
  let text = "";
  let size = 0;

  while (true) {
    const { done, value } = await reader.read();

    if (done) break;

    size += value.length;

    if (size > MAX_CODE_BYTES) {
      await reader.cancel();
      throw new Error("the shared snippet is larger than the playground accepts");
    }

    text += decoder.decode(value, { stream: true });
  }

  return text + decoder.decode();
}
