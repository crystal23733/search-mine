export function unrequestedServiceCall(uri: string, site: string): boolean {
  const url = new URL(uri);
  if (
    url.origin === new URL(site).origin &&
    url.pathname === "/api/v1/auth/bootstrap" &&
    !url.search
  )
    return false;
  return /\/api\/|doubleclick|googlesyndication|accounts\.google\.com|appleid\.apple\.com|kauth\.kakao\.com|nid\.naver\.com/i.test(
    uri,
  );
}
