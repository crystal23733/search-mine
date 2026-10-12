export type Decode<T> = (value: unknown) => T;
export function fail(): never {
  throw Error("malformed");
}
export function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) fail();
  return value as Record<string, unknown>;
}
export function struct<S extends Record<string, Decode<unknown>>>(
  shape: S,
): Decode<{ [K in keyof S]: ReturnType<S[K]> }> {
  return (value) => {
    const source = object(value),
      keys = Object.keys(shape);
    if (
      Object.keys(source).length !== keys.length ||
      keys.some((k) => !Object.hasOwn(source, k))
    )
      fail();
    // Each field is decoded before this generic mapped object is returned.
    return Object.fromEntries(keys.map((k) => [k, shape[k](source[k])])) as {
      [K in keyof S]: ReturnType<S[K]>;
    };
  };
}
export const int =
  (max = Number.MAX_SAFE_INTEGER, min = 0): Decode<number> =>
  (value) =>
    typeof value === "number" &&
    Number.isSafeInteger(value) &&
    value >= min &&
    value <= max
      ? value
      : fail();
export const bool: Decode<boolean> = (v) =>
  typeof v === "boolean" ? v : fail();
export const text =
  (pattern: RegExp): Decode<string> =>
  (v) =>
    typeof v === "string" && pattern.test(v) ? v : fail();
export const uuid = text(
  /^(?!00000000-0000-0000-0000-000000000000$)[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/,
);
export const one =
  <const T extends readonly (string | number)[]>(
    ...values: T
  ): Decode<T[number]> =>
  (v) =>
    values.find((x) => x === v) ?? fail();
export const optional =
  <T>(decode: Decode<T>): Decode<T | null> =>
  (v) =>
    v === null ? null : decode(v);
export const list =
  <T>(decode: Decode<T>, max: number, min = 0): Decode<T[]> =>
  (v) =>
    Array.isArray(v) && v.length >= min && v.length <= max
      ? v.map(decode)
      : fail();
