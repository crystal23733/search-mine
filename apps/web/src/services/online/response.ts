import { MAX_OUTPUT_BYTES } from "@liar/protocol";
import { JsonResponseFailure, readJsonResponse } from "../json-response";
import { OnlineFailure } from "./types";
export async function responseJson(
  response: Response,
  signal: AbortSignal,
  maxBytes = MAX_OUTPUT_BYTES,
): Promise<unknown> {
  try {
    return await readJsonResponse(response, signal, maxBytes);
  } catch (error) {
    if (error instanceof JsonResponseFailure)
      throw new OnlineFailure(error.code);
    throw error;
  }
}
