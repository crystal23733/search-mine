import { isLocale } from "../locale";
export const ROOM_CODE = /^[ABCDEFGHJKLMNPQRSTUVWXYZ23456789]{8}$/;
export function roomCode(value: string): string | null {
  const trimmed = value.trim();
  if (!/^[A-Za-z2-9]{8}$/.test(trimmed)) return null;
  const code = trimmed.toUpperCase();
  return ROOM_CODE.test(code) ? code : null;
}
export function roomInvitation(
  origin: string,
  locale: string,
  code: string,
): string {
  const source = new URL(origin);
  if (
    !isLocale(locale) ||
    !ROOM_CODE.test(code) ||
    !["https:", "http:"].includes(source.protocol)
  )
    throw Error("invalid_invitation");
  const url = new URL(`/${locale}/friends`, source.origin);
  url.searchParams.set("code", code);
  return url.href;
}
