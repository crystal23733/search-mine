import { MAX_OUTPUT_BYTES } from "@liar/protocol";
import { OnlineFailure } from "./types";
export async function responseJson(
  response: Response,
  signal: AbortSignal,
  maxBytes = MAX_OUTPUT_BYTES,
): Promise<unknown> {
  if (
    response.redirected ||
    response.headers.get("content-type")?.toLowerCase().split(";")[0].trim() !==
      "application/json" ||
    !response.body
  )
    throw new OnlineFailure("unavailable");
  const reader = response.body.getReader(),
    chunks: Uint8Array[] = [];
  let size = 0;
  const abort = () => {
    void reader.cancel().catch(() => {});
  };
  signal.addEventListener("abort", abort, { once: true });
  try {
    if (signal.aborted) throw new OnlineFailure("cancelled");
    for (;;) {
      const part = await reader.read();
      if (signal.aborted) throw new OnlineFailure("cancelled");
      if (part.done) break;
      size += part.value.byteLength;
      if (size > maxBytes) {
        await reader.cancel();
        throw new OnlineFailure("malformed");
      }
      chunks.push(part.value);
    }
    const bytes = new Uint8Array(size);
    let offset = 0;
    for (const part of chunks) {
      bytes.set(part, offset);
      offset += part.byteLength;
    }
    return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(bytes));
  } finally {
    signal.removeEventListener("abort", abort);
    reader.releaseLock();
  }
}
