export async function compressCode(code: string) {
  const blob = new Blob([code]);

  const stream = blob.stream();
  const compressedStream = stream.pipeThrough(new CompressionStream("gzip"));

  const compressedArrayBuffer = await new Response(compressedStream).arrayBuffer();
  const compressedBytes = new Uint8Array(compressedArrayBuffer);

  return compressedBytes.toBase64();
}

export async function decompressCode(base64: string) {
  const compressedBytes = Uint8Array.fromBase64(base64);

  const blob = new Blob([compressedBytes]);
  const stream = blob.stream().pipeThrough(new DecompressionStream("gzip"));

  const decompressedText = await new Response(stream).text();

  return decompressedText;
}
