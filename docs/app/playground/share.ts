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

export async function decompressCode(base64: string) {
  const compressedBytes = base64ToUint8(base64);

  const blob = new Blob([compressedBytes]);
  const stream = blob.stream().pipeThrough(new DecompressionStream("gzip"));

  const decompressedText = await new Response(stream).text();

  return decompressedText;
}
