function uint8ToBase64(uint8: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < uint8.length; i++) {
    binary += String.fromCharCode(uint8[i]);
  }
  return btoa(binary);
}

function base64ToUint8(base64: string): Uint8Array {
  const binary = atob(base64);
  const len = binary.length;
  const bytes = new Uint8Array(len);
  for (let i = 0; i < len; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
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

  const blob = new Blob([compressedBytes.buffer as ArrayBuffer]);
  const stream = blob.stream().pipeThrough(new DecompressionStream("gzip"));

  const decompressedText = await new Response(stream).text();

  return decompressedText;
}
