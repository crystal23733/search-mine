import { readFileSync } from "node:fs";
import { decodePublicSiteConfig } from "../src/services/public-policy.ts";
export function readSiteConfig(path: string | undefined) {
  if (!path) return undefined;
  const source = readFileSync(path, "utf8");
  if (Buffer.byteLength(source) > 512 * 1024)
    throw Error("Public configuration exceeds 512 KiB");
  return decodePublicSiteConfig(JSON.parse(source));
}
